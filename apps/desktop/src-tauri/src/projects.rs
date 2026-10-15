//! Projects: folders the person adds (no disk scanning), each read through
//! mise from inside that folder. Until mise trusts a folder's config, nothing
//! is loaded from it; instead powermise previews — without executing — what
//! trusting would allow.
use std::{
    fs,
    path::{Path, PathBuf},
};

use powermise_mise::{
    ConfigFile, ConfigPreview, MiseAdapterError, Task, TaskDetails, ToolInventory, TrustStatus,
    looks_secret, preview_project_configs,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::{CommandResult, message, mise_commands::client};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredProject {
    path: PathBuf,
    added_at: u64,
    last_opened_at: u64,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ProjectStore {
    projects: Vec<StoredProject>,
}

fn store_path() -> CommandResult<PathBuf> {
    Ok(crate::layout()?.state.join("projects.json"))
}

fn load_store() -> CommandResult<ProjectStore> {
    match fs::read(store_path()?) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(message),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ProjectStore::default()),
        Err(error) => Err(message(error)),
    }
}

fn save_store(store: &ProjectStore) -> CommandResult<()> {
    let mut bytes = serde_json::to_vec_pretty(store).map_err(message)?;
    bytes.push(b'\n');
    powermise_state::write_atomically(&store_path()?, &bytes).map_err(message)?;
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    path: PathBuf,
    name: String,
    exists: bool,
    last_opened_at: u64,
}

impl From<&StoredProject> for ProjectEntry {
    fn from(project: &StoredProject) -> Self {
        Self {
            name: folder_name(&project.path),
            exists: project.path.is_dir(),
            last_opened_at: project.last_opened_at,
            path: project.path.clone(),
        }
    }
}

/// Saved projects, most recently opened first.
#[tauri::command]
pub fn list_projects() -> CommandResult<Vec<ProjectEntry>> {
    let mut store = load_store()?;
    store
        .projects
        .sort_by(|a, b| b.last_opened_at.cmp(&a.last_opened_at));
    Ok(store.projects.iter().map(ProjectEntry::from).collect())
}

/// Adds a folder: `path` if given (drag and drop), otherwise the system
/// folder picker. Returns `None` when the picker is cancelled.
#[tauri::command]
pub async fn add_project(
    app: AppHandle,
    path: Option<PathBuf>,
) -> CommandResult<Option<ProjectEntry>> {
    let chosen = match path {
        Some(path) => Some(path),
        None => tauri::async_runtime::spawn_blocking(move || {
            app.dialog()
                .file()
                .set_title("Add a project folder")
                .blocking_pick_folder()
                .and_then(|picked| picked.into_path().ok())
        })
        .await
        .map_err(message)?,
    };
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let path =
        fs::canonicalize(&chosen).map_err(|error| format!("{}: {error}", chosen.display()))?;
    if !path.is_dir() {
        return Err(format!("{} is not a folder", path.display()));
    }
    let mut store = load_store()?;
    let now = now_ms();
    if let Some(existing) = store
        .projects
        .iter_mut()
        .find(|project| project.path == path)
    {
        existing.last_opened_at = now;
    } else {
        store.projects.push(StoredProject {
            path: path.clone(),
            added_at: now,
            last_opened_at: now,
        });
    }
    save_store(&store)?;
    let entry = store
        .projects
        .iter()
        .find(|project| project.path == path)
        .map(ProjectEntry::from);
    Ok(entry)
}

/// Removes a folder from the list. Nothing on disk changes.
#[tauri::command]
pub fn forget_project(path: PathBuf) -> CommandResult<()> {
    let mut store = load_store()?;
    store.projects.retain(|project| project.path != path);
    save_store(&store)
}

/// Records that a project was opened, for the recents order. Best effort.
fn mark_opened(path: &Path) {
    let Ok(mut store) = load_store() else {
        return;
    };
    if let Some(project) = store
        .projects
        .iter_mut()
        .find(|project| project.path == path)
    {
        project.last_opened_at = now_ms();
        let _ = save_store(&store);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarView {
    name: String,
    value: String,
    source: Option<PathBuf>,
    /// Marked `redact = true` in mise, or named like a secret.
    secret: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOverview {
    path: PathBuf,
    name: String,
    trust: TrustStatus,
    /// What trusting would allow, read without executing (untrusted only).
    previews: Vec<ConfigPreview>,
    tools: Option<ToolInventory>,
    config_files: Option<Vec<ConfigFile>>,
    tasks: Option<Vec<Task>>,
    env: Option<Vec<EnvVarView>>,
    /// Reads that failed for reasons other than trust, in words.
    problems: Vec<String>,
}

/// Everything mise reports for a project, read from inside it.
#[tauri::command]
pub async fn project_overview(path: PathBuf) -> CommandResult<ProjectOverview> {
    tauri::async_runtime::spawn_blocking(move || {
        if !path.is_dir() {
            return Err(format!("{} no longer exists", path.display()));
        }
        mark_opened(&path);
        let client = client()?;
        let trust = client.trust_status(&path).map_err(message)?;
        let mut overview = ProjectOverview {
            name: folder_name(&path),
            previews: Vec::new(),
            tools: None,
            config_files: None,
            tasks: None,
            env: None,
            problems: Vec::new(),
            trust,
            path: path.clone(),
        };
        if !overview.trust.trusted {
            overview.previews = preview_project_configs(&path);
            return Ok(overview);
        }

        let mut note = |label: &str, error: MiseAdapterError| {
            let detail = match error {
                MiseAdapterError::Untrusted { .. } => {
                    "a config file here isn't trusted yet".to_owned()
                }
                other => other.to_string(),
            };
            overview.problems.push(format!("{label}: {detail}"));
        };
        match client.tools(Some(&path)) {
            Ok(tools) => overview.tools = Some(tools),
            Err(error) => note("Tools", error),
        }
        match client.config_files(Some(&path)) {
            Ok(files) => overview.config_files = Some(files),
            Err(error) => note("Config files", error),
        }
        match client.tasks(&path) {
            Ok(tasks) => {
                overview.tasks = Some(tasks.into_iter().filter(|task| !task.hide).collect())
            }
            Err(error) => note("Tasks", error),
        }
        match client.env_extended(&path) {
            Ok(env) => {
                let redacted = client.redacted_env_names(&path).unwrap_or_default();
                overview.env = Some(
                    env.into_iter()
                        .filter(|(name, _)| name != "PATH")
                        .map(|(name, var)| EnvVarView {
                            secret: redacted.contains(&name) || looks_secret(&name),
                            value: var.value,
                            source: var.source,
                            name,
                        })
                        .collect(),
                );
            }
            Err(error) => note("Environment", error),
        }
        Ok(overview)
    })
    .await
    .map_err(message)?
}

/// A task's arguments, flags, and choices, for the run form.
#[tauri::command]
pub async fn task_details(path: PathBuf, task: String) -> CommandResult<TaskDetails> {
    tauri::async_runtime::spawn_blocking(move || {
        client()?.task_details(&path, &task).map_err(message)
    })
    .await
    .map_err(message)?
}
