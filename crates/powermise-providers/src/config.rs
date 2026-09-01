//! Typed configuration adapters with managed blocks, snapshots, and drift checks.
use std::{
    fs,
    path::{Path, PathBuf},
};

use powermise_domain::{
    ConfigAdapterApplyOutput, ConfigAdapterCheckOutput, ConfigAdapterRequest, ConfigAdapterState,
    ConfigMutator, ManagedBlock,
};
use powermise_state::{AtomicWriteResult, content_fingerprint, write_atomically};
use serde_json::{Map, Value, json};
use thiserror::Error;

/// The reserved top-level object that owns all Powermise-managed JSON blocks.
const MANAGED_OBJECT_KEY: &str = "_powermise";

#[derive(Debug, Error)]
pub enum ConfigAdapterError {
    #[error("could not read `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: powermise_state::StateError,
    },
    #[error("managed block for `{resource_id}` must contain a valid JSON value")]
    InvalidManagedBlock { resource_id: String },
    #[error("could not parse configuration file `{path}` as JSON: {source}")]
    InvalidJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// A generic JSON managed-block adapter.
///
/// The managed block is stored under the reserved top-level `_powermise` object,
/// keyed by selection marker, so user-owned keys are never rewritten.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsonManagedAdapter;

impl JsonManagedAdapter {
    fn read_object(&self, path: &Path) -> Result<Option<Map<String, Value>>, ConfigAdapterError> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(ConfigAdapterError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let object = serde_json::from_slice::<Value>(&bytes).map_err(|source| {
            ConfigAdapterError::InvalidJson {
                path: path.to_path_buf(),
                source,
            }
        })?;
        match object {
            Value::Object(map) => Ok(Some(map)),
            _ => Err(ConfigAdapterError::InvalidJson {
                path: path.to_path_buf(),
                source: serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "configuration file is not a JSON object",
                )),
            }),
        }
    }

    fn desired_value(&self, request: &ConfigAdapterRequest) -> Result<Value, ConfigAdapterError> {
        serde_json::from_str::<Value>(&request.managed_block.contents)
            .map_err(|_| ConfigAdapterError::InvalidManagedBlock {
                resource_id: request.resource_id.clone(),
            })
    }

    fn record<'a>(
        &self,
        object: &'a Map<String, Value>,
        marker: &str,
    ) -> Option<&'a Map<String, Value>> {
        object
            .get(MANAGED_OBJECT_KEY)
            .and_then(Value::as_object)
            .and_then(|managed| managed.get(marker))
            .and_then(Value::as_object)
    }

    /// Returns the serialized file that would result from applying the managed block.
    fn reconcile(&self, request: &ConfigAdapterRequest) -> Result<Vec<u8>, ConfigAdapterError> {
        let mut object = self.read_object(&request.path)?.unwrap_or_default();
        let desired = self.desired_value(request)?;
        let managed = object
            .entry(MANAGED_OBJECT_KEY.to_owned())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("MANAGED_OBJECT_KEY is written as an object");
        managed.insert(
            request.managed_block.marker.clone(),
            json!({ "desired": desired }),
        );
        Ok(serde_json::to_vec_pretty(&Value::Object(object))
            .expect("serializing a JSON object cannot fail"))
    }
}

impl ConfigMutator for JsonManagedAdapter {
    type Error = ConfigAdapterError;

    fn check(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterCheckOutput, Self::Error> {
        let bytes = match fs::read(&request.path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ConfigAdapterCheckOutput {
                    state: ConfigAdapterState::Missing,
                    detail: format!(
                        "configuration file `{}` does not exist",
                        request.path.display()
                    ),
                });
            }
            Err(source) => {
                return Err(ConfigAdapterError::Read {
                    path: request.path.to_path_buf(),
                    source,
                });
            }
        };
        let object = serde_json::from_slice::<Value>(&bytes)
            .map_err(|source| ConfigAdapterError::InvalidJson {
                path: request.path.clone(),
                source,
            })?
            .as_object()
            .cloned()
            .ok_or_else(|| ConfigAdapterError::InvalidJson {
                path: request.path.clone(),
                source: serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "configuration file is not a JSON object",
                )),
            })?;
        let record = self.record(&object, &request.managed_block.marker);
        if let Some(record) = record {
            let desired = self.desired_value(request)?;
            if record.get("desired") == Some(&desired) {
                if let Some(expected) = &request.applied_fingerprint {
                    let actual = content_fingerprint(&bytes);
                    if *expected != actual {
                        return Ok(ConfigAdapterCheckOutput {
                            state: ConfigAdapterState::Drifted,
                            detail: format!(
                                "file `{}` changed since the last apply (expected fingerprint {expected}, got {actual})",
                                request.path.display()
                            ),
                        });
                    }
                }
                return Ok(ConfigAdapterCheckOutput {
                    state: ConfigAdapterState::Current,
                    detail: format!(
                        "managed block for `{}` is present and current",
                        request.managed_block.marker
                    ),
                });
            }
        }
        Ok(ConfigAdapterCheckOutput {
            state: ConfigAdapterState::Missing,
            detail: format!(
                "managed block for `{}` is absent or out of date",
                request.managed_block.marker
            ),
        })
    }

    fn plan(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterApplyOutput, Self::Error> {
        if self.check(request)?.state == ConfigAdapterState::Current {
            return Ok(ConfigAdapterApplyOutput {
                changed: false,
                before_fingerprint: request.applied_fingerprint.clone(),
                after_fingerprint: request.applied_fingerprint.clone().unwrap_or_default(),
            });
        }
        let bytes = self.reconcile(request)?;
        Ok(ConfigAdapterApplyOutput {
            changed: true,
            before_fingerprint: fs::read(&request.path)
                .ok()
                .map(|bytes| content_fingerprint(&bytes)),
            after_fingerprint: content_fingerprint(&bytes),
        })
    }

    fn apply(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterApplyOutput, Self::Error> {
        let before_fingerprint = fs::read(&request.path)
            .ok()
            .map(|bytes| content_fingerprint(&bytes));
        let bytes = self.reconcile(request)?;
        let after_fingerprint = content_fingerprint(&bytes);
        let result = write_atomically(&request.path, &bytes).map_err(|source| {
            ConfigAdapterError::Write {
                path: request.path.clone(),
                source,
            }
        })?;
        Ok(ConfigAdapterApplyOutput {
            changed: result != AtomicWriteResult::Unchanged,
            before_fingerprint,
            after_fingerprint,
        })
    }

    fn restore(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterApplyOutput, Self::Error> {
        let before_fingerprint = fs::read(&request.path)
            .ok()
            .map(|bytes| content_fingerprint(&bytes));
        let mut object = match self.read_object(&request.path)? {
            Some(object) => object,
            None => {
                let after_fingerprint = before_fingerprint.clone().unwrap_or_default();
                return Ok(ConfigAdapterApplyOutput {
                    changed: false,
                    before_fingerprint,
                    after_fingerprint,
                });
            }
        };
        let Some(managed) = object
            .get_mut(MANAGED_OBJECT_KEY)
            .and_then(Value::as_object_mut)
        else {
            let after_fingerprint = before_fingerprint.clone().unwrap_or_default();
            return Ok(ConfigAdapterApplyOutput {
                changed: false,
                before_fingerprint,
                after_fingerprint,
            });
        };
        let removed = managed.remove(&request.managed_block.marker).is_some();
        if managed.is_empty() {
            object.remove(MANAGED_OBJECT_KEY);
        }
        let bytes = serde_json::to_vec_pretty(&Value::Object(object))
            .expect("serializing a JSON object cannot fail");
        let after_fingerprint = content_fingerprint(&bytes);
        let result = if removed {
            write_atomically(&request.path, &bytes).map_err(|source| {
                ConfigAdapterError::Write {
                    path: request.path.clone(),
                    source,
                }
            })?
        } else {
            AtomicWriteResult::Unchanged
        };
        Ok(ConfigAdapterApplyOutput {
            changed: result != AtomicWriteResult::Unchanged,
            before_fingerprint,
            after_fingerprint,
        })
    }
}

/// A generic text managed-block adapter using comment markers, suitable for TOML
/// and other line-oriented configuration documents.
#[derive(Clone, Copy, Debug, Default)]
pub struct ManagedBlockAdapter;

impl ManagedBlockAdapter {
    fn begin_marker(marker: &str) -> String {
        format!("# --- BEGIN powermise managed: {marker} ---")
    }

    fn end_marker(marker: &str) -> String {
        format!("# --- END powermise managed: {marker} ---")
    }

    fn block(&self, marker: &str, contents: &str) -> String {
        format!(
            "{}\n{}\n{}",
            Self::begin_marker(marker),
            contents.trim_end(),
            Self::end_marker(marker)
        )
    }

    fn read_document(&self, path: &Path) -> Result<Option<String>, ConfigAdapterError> {
        match fs::read_to_string(path) {
            Ok(contents) => Ok(Some(contents)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(ConfigAdapterError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    fn locate(
        &self,
        document: &str,
        marker: &str,
    ) -> Option<(usize, usize)> {
        let begin = Self::begin_marker(marker);
        let end = Self::end_marker(marker);
        let begin_index = document.find(&begin)?;
        let after_begin = begin_index + begin.len();
        let end_index = document[after_begin..].find(&end)? + after_begin;
        Some((begin_index, end_index + end.len()))
    }

    fn reconcile(&self, request: &ConfigAdapterRequest) -> Result<Vec<u8>, ConfigAdapterError> {
        let mut document = self.read_document(&request.path)?.unwrap_or_default();
        let block = self.block(&request.managed_block.marker, &request.managed_block.contents);
        if let Some((start, end)) = self.locate(&document, &request.managed_block.marker) {
            document.replace_range(start..end, &block);
        } else {
            if !document.is_empty() && !document.ends_with('\n') {
                document.push('\n');
            }
            document.push_str(&block);
            document.push('\n');
        }
        Ok(document.into_bytes())
    }
}

impl ConfigMutator for ManagedBlockAdapter {
    type Error = ConfigAdapterError;

    fn check(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterCheckOutput, Self::Error> {
        let Some(document) = self.read_document(&request.path)? else {
            return Ok(ConfigAdapterCheckOutput {
                state: ConfigAdapterState::Missing,
                detail: format!("configuration file `{}` does not exist", request.path.display()),
            });
        };
        let block = self.block(&request.managed_block.marker, &request.managed_block.contents);
        let Some((start, end)) = self.locate(&document, &request.managed_block.marker) else {
            return Ok(ConfigAdapterCheckOutput {
                state: ConfigAdapterState::Missing,
                detail: format!(
                    "managed block for `{}` is absent or out of date",
                    request.managed_block.marker
                ),
            });
        };
        if document[start..end] != block {
            return Ok(ConfigAdapterCheckOutput {
                state: ConfigAdapterState::Missing,
                detail: format!(
                    "managed block for `{}` is absent or out of date",
                    request.managed_block.marker
                ),
            });
        }
        if let Some(expected) = &request.applied_fingerprint {
            let actual = content_fingerprint(document.as_bytes());
            if *expected != actual {
                return Ok(ConfigAdapterCheckOutput {
                    state: ConfigAdapterState::Drifted,
                    detail: format!(
                        "file `{}` changed since the last apply",
                        request.path.display()
                    ),
                });
            }
        }
        Ok(ConfigAdapterCheckOutput {
            state: ConfigAdapterState::Current,
            detail: format!(
                "managed block for `{}` is present and current",
                request.managed_block.marker
            ),
        })
    }

    fn plan(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterApplyOutput, Self::Error> {
        if self.check(request)?.state == ConfigAdapterState::Current {
            return Ok(ConfigAdapterApplyOutput {
                changed: false,
                before_fingerprint: request.applied_fingerprint.clone(),
                after_fingerprint: request.applied_fingerprint.clone().unwrap_or_default(),
            });
        }
        let bytes = self.reconcile(request)?;
        Ok(ConfigAdapterApplyOutput {
            changed: true,
            before_fingerprint: fs::read(&request.path)
                .ok()
                .map(|bytes| content_fingerprint(&bytes)),
            after_fingerprint: content_fingerprint(&bytes),
        })
    }

    fn apply(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterApplyOutput, Self::Error> {
        let before_fingerprint = fs::read(&request.path)
            .ok()
            .map(|bytes| content_fingerprint(&bytes));
        let bytes = self.reconcile(request)?;
        let after_fingerprint = content_fingerprint(&bytes);
        let result = write_atomically(&request.path, &bytes).map_err(|source| {
            ConfigAdapterError::Write {
                path: request.path.clone(),
                source,
            }
        })?;
        Ok(ConfigAdapterApplyOutput {
            changed: result != AtomicWriteResult::Unchanged,
            before_fingerprint,
            after_fingerprint,
        })
    }

    fn restore(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterApplyOutput, Self::Error> {
        let before_fingerprint = fs::read(&request.path)
            .ok()
            .map(|bytes| content_fingerprint(&bytes));
        let Some(mut document) = self.read_document(&request.path)? else {
            return Ok(ConfigAdapterApplyOutput {
                changed: false,
                before_fingerprint: None,
                after_fingerprint: before_fingerprint.unwrap_or_default(),
            });
        };
        let marker = request.managed_block.marker.clone();
        let removed = match self.locate(&document, &marker) {
            Some((start, end)) => {
                document.replace_range(start..end, "");
                true
            }
            None => false,
        };
        let bytes = document.into_bytes();
        let after_fingerprint = content_fingerprint(&bytes);
        let result = if removed {
            write_atomically(&request.path, &bytes).map_err(|source| {
                ConfigAdapterError::Write {
                    path: request.path.clone(),
                    source,
                }
            })?
        } else {
            AtomicWriteResult::Unchanged
        };
        Ok(ConfigAdapterApplyOutput {
            changed: result != AtomicWriteResult::Unchanged,
            before_fingerprint,
            after_fingerprint,
        })
    }
}

/// A Claude Code MCP server entry managed in `settings.json`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeMcpServer {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
}

/// Builds the Claude harness managed block: the GitHub app integration and MCP servers.
#[derive(Clone, Debug)]
pub struct ClaudeSettingsAdapter {
    selection: String,
    github_integration: bool,
    mcp_servers: Vec<ClaudeMcpServer>,
}

impl ClaudeSettingsAdapter {
    pub fn new(selection: impl Into<String>) -> Self {
        Self {
            selection: selection.into(),
            github_integration: false,
            mcp_servers: Vec::new(),
        }
    }

    pub fn with_github_integration(mut self, enabled: bool) -> Self {
        self.github_integration = enabled;
        self
    }

    pub fn with_mcp_server(mut self, server: ClaudeMcpServer) -> Self {
        self.mcp_servers.push(server);
        self
    }

    pub fn managed_block(&self) -> ManagedBlock {
        let mut root = Map::new();
        if self.github_integration {
            root.insert("github".into(), json!({ "enabled": true }));
        }
        let mut servers = Map::new();
        for server in &self.mcp_servers {
            servers.insert(
                server.id.clone(),
                json!({ "command": server.command, "args": server.args }),
            );
        }
        root.insert("mcpServers".into(), Value::Object(servers));
        ManagedBlock {
            marker: self.selection.clone(),
            contents: serde_json::to_string_pretty(&Value::Object(root))
                .expect("serializing a JSON object cannot fail"),
        }
    }

    pub fn request(
        &self,
        config_dir: &Path,
        applied_fingerprint: Option<String>,
    ) -> ConfigAdapterRequest {
        ConfigAdapterRequest {
            config_target: "claude".into(),
            resource_id: format!("config-adapter:harness:{}", self.selection),
            path: claude_settings_path(config_dir),
            managed_block: self.managed_block(),
            applied_fingerprint,
        }
    }
}

/// Builds the VS Code editor managed block for a managed preference key.
#[derive(Clone, Debug)]
pub struct VscodeSettingsAdapter {
    selection: String,
}

impl VscodeSettingsAdapter {
    pub fn new(selection: impl Into<String>) -> Self {
        Self {
            selection: selection.into(),
        }
    }

    pub fn managed_block(&self) -> ManagedBlock {
        ManagedBlock {
            marker: self.selection.clone(),
            contents: serde_json::to_string_pretty(&json!({
                "editor.minimap": { "enabled": false }
            }))
            .expect("serializing a JSON object cannot fail"),
        }
    }

    pub fn request(
        &self,
        config_dir: &Path,
        applied_fingerprint: Option<String>,
    ) -> ConfigAdapterRequest {
        ConfigAdapterRequest {
            config_target: "vscode".into(),
            resource_id: format!("config-adapter:editor:{}", self.selection),
            path: vscode_settings_path(config_dir),
            managed_block: self.managed_block(),
            applied_fingerprint,
        }
    }
}

/// The Claude Code settings file under a configuration directory.
pub fn claude_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(".claude/settings.json")
}

/// The VS Code settings file under a configuration directory.
pub fn vscode_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join("Code/User/settings.json")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "powermise-config-test-{}-{timestamp}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test directory should be created");
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn claude_request(
        config_dir: &Path,
        applied_fingerprint: Option<String>,
    ) -> ConfigAdapterRequest {
        ClaudeSettingsAdapter::new("engineering-agents")
            .with_github_integration(true)
            .with_mcp_server(ClaudeMcpServer {
                id: "context7".into(),
                command: "npx".into(),
                args: vec!["-y".into(), "@upstash/context7-mcp".into()],
            })
            .request(config_dir, applied_fingerprint)
    }

    #[test]
    fn claude_apply_is_idempotent_and_drifts_are_detected() {
        let directory = TestDirectory::new();
        let adapter = JsonManagedAdapter;
        let path = claude_settings_path(&directory.path);

        let first = adapter.apply(&claude_request(&directory.path, None)).unwrap();
        assert!(first.changed);
        assert!(first.before_fingerprint.is_none());
        let fingerprint = first.after_fingerprint;
        assert_eq!(fs::read(&path).unwrap(), adapter.reconcile(&claude_request(&directory.path, None)).unwrap());

        let second = adapter
            .apply(&claude_request(&directory.path, Some(fingerprint.clone())))
            .unwrap();
        assert!(!second.changed);

        let check = adapter
            .check(&claude_request(&directory.path, Some(fingerprint.clone())))
            .unwrap();
        assert_eq!(check.state, ConfigAdapterState::Current);

        let mut drifted_object: Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        drifted_object
            .as_object_mut()
            .unwrap()
            .insert("userKey".into(), json!(true));
        fs::write(&path, serde_json::to_vec_pretty(&drifted_object).unwrap()).unwrap();
        let drifted = adapter
            .check(&claude_request(&directory.path, Some(fingerprint)))
            .unwrap();
        assert_eq!(drifted.state, ConfigAdapterState::Drifted);
    }

    #[test]
    fn claude_restore_removes_only_the_managed_block() {
        let directory = TestDirectory::new();
        let adapter = JsonManagedAdapter;
        let path = claude_settings_path(&directory.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"userKey\": true}").unwrap();

        let applied = adapter.apply(&claude_request(&directory.path, None)).unwrap();
        assert!(applied.changed);

        let restored = adapter.restore(&claude_request(&directory.path, None)).unwrap();
        assert!(restored.changed);
        let contents = fs::read_to_string(&path).unwrap();
        let object: Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(object["userKey"], json!(true));
        assert!(object.get("_powermise").is_none());
    }

    #[test]
    fn managed_block_adapter_round_trips_text_documents() {
        let directory = TestDirectory::new();
        let adapter = ManagedBlockAdapter;
        let path = directory.path.join("mise.toml");
        fs::write(&path, "[settings]\nvalue = 1\n").unwrap();

        let block = ManagedBlock {
            marker: "engineering-agents".into(),
            contents: "[tools]\nnode = \"24\"\n".into(),
        };
        let request = ConfigAdapterRequest {
            config_target: "mise".into(),
            resource_id: "config-adapter:test".into(),
            path: path.clone(),
            managed_block: block.clone(),
            applied_fingerprint: None,
        };

        assert!(adapter.apply(&request).unwrap().changed);
        let document = fs::read_to_string(&path).unwrap();
        assert!(document.contains("# --- BEGIN powermise managed: engineering-agents ---"));
        assert!(document.contains("[tools]\nnode = \"24\""));

        assert_eq!(
            adapter.check(&request).unwrap().state,
            ConfigAdapterState::Current
        );
        let applied = adapter.apply(&request).unwrap();
        assert!(!applied.changed);

        assert!(adapter.restore(&request).unwrap().changed);
        let restored = fs::read_to_string(&path).unwrap();
        assert!(!restored.contains("powermise managed"));
        assert!(restored.contains("[settings]\nvalue = 1"));
    }

    #[test]
    fn invalid_managed_json_is_rejected() {
        let directory = TestDirectory::new();
        let adapter = JsonManagedAdapter;
        let request = ConfigAdapterRequest {
            config_target: "claude".into(),
            resource_id: "config-adapter:test".into(),
            path: claude_settings_path(&directory.path),
            managed_block: ManagedBlock {
                marker: "engineering-agents".into(),
                contents: "not json".into(),
            },
            applied_fingerprint: None,
        };

        assert!(matches!(
            adapter.apply(&request),
            Err(ConfigAdapterError::InvalidManagedBlock { .. })
        ));
    }
}