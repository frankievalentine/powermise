//! Projects: folders the person adds (no disk scanning), each read through
//! mise from inside that folder. Until mise trusts a folder's config, nothing
//! is loaded from it; instead powermise previews — without executing — what
//! trusting would allow.
use std::{
    fs,
    path::{Path, PathBuf},
};

use powermise_mise::{
    ConfigFile, ConfigPreview, Generator, LockChange, MiseAdapterError, SecretsReport, Task,
    TaskDetails, ToolInventory, TrustStatus, VarDeclaration, VarEntry, declared_vars,
    idiomatic_files, looks_secret, preview_project_configs, project_environments,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUpdates {
    path: PathBuf,
    name: String,
    outdated: powermise_mise::OutdatedReport,
}

/// For every saved, trusted project: the tools its own config asks for that
/// have a newer version (`mise outdated --local --bump --json`). Untrusted or
/// missing folders are skipped.
#[tauri::command]
pub async fn project_updates() -> CommandResult<Vec<ProjectUpdates>> {
    tauri::async_runtime::spawn_blocking(|| {
        let client = crate::mise_commands::client()?;
        let store = load_store()?;
        Ok(store
            .projects
            .iter()
            .filter(|project| project.path.is_dir())
            .filter_map(|project| {
                let outdated = client.outdated_local(&project.path).ok()?;
                (!outdated.is_empty()).then(|| ProjectUpdates {
                    name: folder_name(&project.path),
                    path: project.path.clone(),
                    outdated,
                })
            })
            .collect())
    })
    .await
    .map_err(message)?
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
    /// Environments the project has config for (`mise.<env>.toml`).
    environments: Vec<String>,
    /// The environment this overview previews (`MISE_ENV`), if any.
    environment: Option<String>,
    /// `.nvmrc` and friends, and whether mise reads each one.
    idiomatic: Vec<IdiomaticView>,
    /// The lockfile, when the project has one.
    lockfile: Option<LockfileView>,
    /// `[vars]` (mise 2026.10.6+), or `None` on older mise.
    vars: Option<Vec<VarView>>,
    /// Secret names and their source (experimental in mise; never values).
    secrets: Option<SecretsReport>,
    /// Whether the folder is a Git repository (for the workflow and hook
    /// generators, which need one).
    is_git_repo: bool,
    /// Reads that failed for reasons other than trust, in words.
    problems: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdiomaticView {
    file: String,
    tool: String,
    version: String,
    /// mise reads it: the tool is in `idiomatic_version_file_enable_tools`
    /// and the file isn't in `idiomatic_version_file_disable_files`.
    enabled: bool,
    /// Turned off on its own (`tool:file` in
    /// `idiomatic_version_file_disable_files`), with the list's other entries
    /// so turning it back on can keep them.
    disabled_file: bool,
    disabled_files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VarView {
    name: String,
    /// `None` when a required var has no value yet.
    value: Option<String>,
    source: Option<String>,
    /// The value was saved on this computer (`mise vars NAME=VALUE`).
    saved: bool,
    /// Declared with a prompt, default, or `required`, so a saved value
    /// applies; plain `[vars]` entries are fixed by the config.
    settable: bool,
    prompt: Option<String>,
    required: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileView {
    path: PathBuf,
    /// What `mise lock` would change; empty means up to date.
    changes: Vec<LockChange>,
}

/// Everything mise reports for a project, read from inside it.
#[tauri::command]
pub async fn project_overview(
    path: PathBuf,
    environment: Option<String>,
) -> CommandResult<ProjectOverview> {
    tauri::async_runtime::spawn_blocking(move || {
        if !path.is_dir() {
            return Err(format!("{} no longer exists", path.display()));
        }
        mark_opened(&path);
        let environments = project_environments(&path);
        // Only preview environments the project has config for.
        let environment = environment.filter(|name| environments.contains(name));
        let base = client()?;
        let client = base
            .clone()
            .with_environment(environment.as_deref())
            .map_err(message)?;
        let trust = base.trust_status(&path).map_err(message)?;
        let mut overview = ProjectOverview {
            name: folder_name(&path),
            previews: Vec::new(),
            tools: None,
            config_files: None,
            tasks: None,
            env: None,
            environments,
            environment,
            idiomatic: Vec::new(),
            lockfile: None,
            vars: None,
            secrets: None,
            is_git_repo: path.join(".git").exists(),
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

        let found = idiomatic_files(&path);
        if !found.is_empty() {
            let settings = base.settings().unwrap_or_default();
            let list = |key: &str| -> Vec<String> {
                settings
                    .iter()
                    .find(|setting| setting.key == key)
                    .and_then(|setting| serde_json::from_value(setting.value.clone()).ok())
                    .unwrap_or_default()
            };
            let enabled = list("idiomatic_version_file_enable_tools");
            let disabled = list("idiomatic_version_file_disable_files");
            overview.idiomatic = found
                .into_iter()
                .map(|file| {
                    let disabled_file = disabled.contains(&format!("{}:{}", file.tool, file.file));
                    IdiomaticView {
                        enabled: enabled.contains(&file.tool) && !disabled_file,
                        disabled_file,
                        disabled_files: disabled.clone(),
                        file: file.file,
                        tool: file.tool,
                        version: file.version,
                    }
                })
                .collect();
        }

        let lockfile = ["mise.lock", "mise.local.lock"]
            .iter()
            .map(|name| path.join(name))
            .find(|candidate| candidate.is_file());
        if let Some(lockfile) = lockfile {
            match client.lock_changes(&path) {
                Ok(changes) => {
                    overview.lockfile = Some(LockfileView {
                        path: lockfile,
                        changes,
                    })
                }
                Err(error) => note("Lockfile", error),
            }
        }

        if crate::mise_commands::mise_version(&base)
            .is_some_and(|version| version >= powermise_mise::MiseVersion::VARS)
        {
            match client.vars(&path) {
                Ok(listed) => overview.vars = Some(merge_vars(listed, declared_vars(&path), &path)),
                Err(error) => note("Vars", error),
            }
            // Experimental in mise: shown only when it reports something.
            overview.secrets = client
                .secrets(&path)
                .ok()
                .filter(|report| report.source.is_some() || !report.keys.is_empty());
        }
        Ok(overview)
    })
    .await
    .map_err(message)?
}

/// Joins what `mise vars ls` reports with the declarations in the config:
/// declared vars take a saved value, and required ones with no value yet are
/// listed too.
fn merge_vars(
    listed: Vec<VarEntry>,
    declared: Vec<VarDeclaration>,
    project: &Path,
) -> Vec<VarView> {
    let in_project = |source: &str| {
        let home = crate::home_dir().map(|home| home.to_string_lossy().into_owned());
        let expanded = match (source.strip_prefix('~'), home) {
            (Some(rest), Some(home)) => format!("{home}{rest}"),
            _ => source.to_owned(),
        };
        Path::new(&expanded).starts_with(project)
    };
    let mut views: Vec<VarView> = listed
        .into_iter()
        .map(|entry| {
            let declaration = declared.iter().find(|var| var.name == entry.name);
            let saved = entry
                .source
                .as_deref()
                .is_some_and(|source| source.ends_with("vars.toml") && !in_project(source));
            VarView {
                settable: declaration.is_some(),
                prompt: declaration.and_then(|var| var.prompt.clone()),
                required: declaration.is_some_and(|var| var.required),
                saved,
                value: Some(entry.value),
                source: entry.source,
                name: entry.name,
            }
        })
        .collect();
    for declaration in declared {
        if !views.iter().any(|view| view.name == declaration.name) {
            views.push(VarView {
                name: declaration.name,
                value: None,
                source: Some(declaration.file.to_string_lossy().into_owned()),
                saved: false,
                settable: true,
                prompt: declaration.prompt,
                required: declaration.required,
            });
        }
    }
    views
}

/// Opens the system terminal in a project, running `mise en` (a shell with
/// the project's tools and environment) or `mise run <task>` for tasks that
/// need a real terminal.
#[tauri::command]
pub fn open_terminal(path: PathBuf, task: Option<String>) -> CommandResult<()> {
    if !path.is_dir() {
        return Err(format!("{} no longer exists", path.display()));
    }
    // Task names are passed to a shell here, so keep them to plain names.
    if let Some(task) = &task
        && (task.is_empty()
            || task.starts_with('-')
            || !task
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ":_.-/".contains(character)))
    {
        return Err(format!("`{task}` can't be opened in a terminal"));
    }
    let mise = crate::find_mise().ok_or("mise is not installed")?;
    let arguments: Vec<String> = match &task {
        Some(task) => vec!["run".into(), task.clone()],
        None => vec!["en".into()],
    };
    crate::terminal::open(&path, &mise, &arguments).map_err(message)
}

/// The file `mise generate <kind>` would add, without writing it.
#[tauri::command]
pub async fn generate_preview(
    path: PathBuf,
    generator: Generator,
    task: Option<String>,
) -> CommandResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        client()?
            .generate_preview(&path, generator, task.as_deref())
            .map_err(message)
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
