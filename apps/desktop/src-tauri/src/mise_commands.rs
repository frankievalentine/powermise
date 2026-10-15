//! The mise companion: read what mise manages through its JSON commands, and
//! change it only by running mise itself. Changes run as background jobs that
//! stream output to the window, can be cancelled, and are kept in an activity
//! log with the exact command line.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use powermise_mise::{
    Doctor, MiseClient, MiseCommand, MiseVersion, OutdatedReport, RegistryEntry, RemoteVersion,
    RunOutcome, ToolInventory,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{CommandResult, find_mise, home_dir, message};

/// Running jobs, the activity log, and the cached registry.
#[derive(Default)]
pub struct MiseState {
    next_job: AtomicU64,
    running: Mutex<HashMap<u64, Arc<AtomicBool>>>,
    activity: Mutex<Vec<ActivityEntry>>,
    registry: Mutex<Option<Vec<RegistryEntry>>>,
}

fn client() -> CommandResult<MiseClient> {
    let executable = find_mise().ok_or_else(|| {
        "mise is not installed. Install it from https://mise.jdx.dev, then try again.".to_owned()
    })?;
    let home = home_dir().ok_or("could not determine your home directory")?;
    Ok(MiseClient::new(executable, home))
}

/// Runs blocking mise work off the main thread.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> CommandResult<T> + Send + 'static,
) -> CommandResult<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(message)?
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiseInfo {
    path: Option<PathBuf>,
    version: Option<String>,
    latest: Option<String>,
    minimum: String,
    /// Whether the installed mise is new enough for Powermise.
    supported: bool,
}

#[tauri::command]
pub async fn mise_info() -> CommandResult<MiseInfo> {
    blocking(|| {
        let minimum = MiseVersion::MINIMUM.to_string();
        let Ok(client) = client() else {
            return Ok(MiseInfo {
                path: None,
                version: None,
                latest: None,
                minimum,
                supported: false,
            });
        };
        let info = client.version_info().ok();
        let supported = info
            .as_ref()
            .and_then(|info| MiseVersion::parse(&info.version))
            .is_some_and(|version| version >= MiseVersion::MINIMUM);
        Ok(MiseInfo {
            path: Some(client.executable().to_path_buf()),
            version: info.as_ref().map(|info| info.version.clone()),
            latest: info.and_then(|info| info.latest),
            minimum,
            supported,
        })
    })
    .await
}

#[tauri::command]
pub async fn installed_tools() -> CommandResult<ToolInventory> {
    blocking(|| client()?.tools(None).map_err(message)).await
}

#[tauri::command]
pub async fn outdated_tools() -> CommandResult<OutdatedReport> {
    blocking(|| client()?.outdated(None).map_err(message)).await
}

#[tauri::command]
pub async fn registry(app: AppHandle) -> CommandResult<Vec<RegistryEntry>> {
    if let Some(cached) = app
        .state::<MiseState>()
        .registry
        .lock()
        .map_err(message)?
        .clone()
    {
        return Ok(cached);
    }
    let entries = blocking(|| client()?.registry().map_err(message)).await?;
    *app.state::<MiseState>().registry.lock().map_err(message)? = Some(entries.clone());
    Ok(entries)
}

#[tauri::command]
pub async fn remote_versions(tool: String) -> CommandResult<Vec<RemoteVersion>> {
    blocking(move || client()?.remote_versions(&tool).map_err(message)).await
}

#[tauri::command]
pub async fn doctor() -> CommandResult<Doctor> {
    blocking(|| client()?.doctor().map_err(message)).await
}

// ---------------------------------------------------------------------------
// Changes
// ---------------------------------------------------------------------------

/// A change the window can ask for. Each maps to exactly one mise command.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MiseAction {
    UseGlobal { tool: String, version: String },
    UnuseGlobal { tool: String },
    Uninstall { tool: String, version: String },
    Upgrade { tool: Option<String> },
    SelfUpdate,
}

impl MiseAction {
    fn command(&self) -> CommandResult<MiseCommand> {
        match self {
            Self::UseGlobal { tool, version } => MiseCommand::use_global(tool, version),
            Self::UnuseGlobal { tool } => MiseCommand::unuse_global(tool),
            Self::Uninstall { tool, version } => MiseCommand::uninstall(tool, version),
            Self::Upgrade { tool } => MiseCommand::upgrade(tool.as_deref()),
            Self::SelfUpdate => Ok(MiseCommand::self_update()),
        }
        .map_err(message)
    }
}

/// The exact command line an action runs, for confirmation sheets.
#[tauri::command]
pub fn describe_action(action: MiseAction) -> CommandResult<String> {
    Ok(action.command()?.display())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    id: u64,
    command: String,
    started_at: u64,
    finished_at: Option<u64>,
    outcome: Option<RunOutcome>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputEvent {
    job_id: u64,
    stream: powermise_mise::Stream,
    text: String,
}

/// Emitted once per output line of a running job.
const OUTPUT_EVENT: &str = "mise-output";
/// Emitted with the finished [`ActivityEntry`] when a job ends.
const FINISHED_EVENT: &str = "mise-finished";

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

/// Starts an action and returns its job id immediately; output and the result
/// arrive as `mise-output` and `mise-finished` events.
#[tauri::command]
pub fn run_action(
    app: AppHandle,
    state: State<'_, MiseState>,
    action: MiseAction,
) -> CommandResult<u64> {
    let client = client()?;
    let command = action.command()?;
    let id = state.next_job.fetch_add(1, Ordering::Relaxed) + 1;
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .running
        .lock()
        .map_err(message)?
        .insert(id, cancel.clone());
    state.activity.lock().map_err(message)?.push(ActivityEntry {
        id,
        command: command.display(),
        started_at: now_ms(),
        finished_at: None,
        outcome: None,
        error: None,
    });

    std::thread::spawn(move || {
        let result = client.run(&command, &cancel, |line| {
            let _ = app.emit(
                OUTPUT_EVENT,
                OutputEvent {
                    job_id: id,
                    stream: line.stream,
                    text: line.text,
                },
            );
        });
        let state = app.state::<MiseState>();
        if let Ok(mut running) = state.running.lock() {
            running.remove(&id);
        }
        // Installs change what the registry view shows as installed, not the
        // registry itself, so the cache stays valid.
        let finished = state.activity.lock().ok().and_then(|mut activity| {
            let entry = activity.iter_mut().find(|entry| entry.id == id)?;
            entry.finished_at = Some(now_ms());
            match result {
                Ok(outcome) => entry.outcome = Some(outcome),
                Err(error) => entry.error = Some(error.to_string()),
            }
            Some(entry.clone())
        });
        if let Some(entry) = finished {
            let _ = app.emit(FINISHED_EVENT, entry);
        }
    });
    Ok(id)
}

#[tauri::command]
pub fn cancel_job(state: State<'_, MiseState>, job_id: u64) -> CommandResult<()> {
    if let Some(cancel) = state.running.lock().map_err(message)?.get(&job_id) {
        cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Every command Powermise ran this session, newest first.
#[tauri::command]
pub fn activity(state: State<'_, MiseState>) -> CommandResult<Vec<ActivityEntry>> {
    let mut entries = state.activity.lock().map_err(message)?.clone();
    entries.reverse();
    Ok(entries)
}
