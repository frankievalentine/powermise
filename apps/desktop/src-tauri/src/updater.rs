//! powermise's own updates, through Tauri's updater: it reads `latest.json`
//! from the newest GitHub release (`plugins.updater` in tauri.conf.json),
//! checks the download against the public key built into the app, and
//! replaces the installed app. Releases sign their update files in CI
//! (.github/workflows/release.yml).
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::{CommandResult, message};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    version: String,
    current: String,
    /// The release's notes.
    notes: Option<String>,
}

/// This build's version.
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// A newer powermise, if there is one.
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> CommandResult<Option<AppUpdate>> {
    let update = app
        .updater()
        .map_err(message)?
        .check()
        .await
        .map_err(message)?;
    Ok(update.map(|update| AppUpdate {
        version: update.version.clone(),
        current: update.current_version.clone(),
        notes: update.body.clone(),
    }))
}

/// Downloads and installs the newer powermise, then restarts into it.
#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> CommandResult<()> {
    let update = app
        .updater()
        .map_err(message)?
        .check()
        .await
        .map_err(message)?
        .ok_or("powermise is already up to date")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(message)?;
    app.restart()
}
