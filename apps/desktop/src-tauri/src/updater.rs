//! powermise's own updates, through Tauri's updater: it reads `latest.json`
//! from the newest GitHub release (`plugins.updater` in tauri.conf.json),
//! checks the download against the public key built into the app, and
//! replaces the installed app. Releases sign their update files in CI
//! (.github/workflows/release.yml).
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
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

/// The menu id of powermise › Check for Updates… (macOS's app menu).
pub const CHECK_MENU_ID: &str = "check-for-updates";

/// Check for Updates… from the menu: native dialogs say the result, and offer
/// to install a newer version.
pub fn check_from_menu(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let found = match app.updater() {
            Ok(updater) => updater.check().await.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let dialog = app.dialog();
        match found {
            Ok(Some(update)) => {
                let app = app.clone();
                dialog
                    .message(format!(
                        "powermise {} is available. You have {}.",
                        update.version, update.current_version
                    ))
                    .title("A new version of powermise is available")
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "Restart to Update".into(),
                        "Later".into(),
                    ))
                    .show(move |install| {
                        if !install {
                            return;
                        }
                        tauri::async_runtime::spawn(async move {
                            match update.download_and_install(|_, _| {}, || {}).await {
                                Ok(()) => app.restart(),
                                Err(error) => {
                                    app.dialog()
                                        .message(format!("The update didn’t install: {error}"))
                                        .title("powermise couldn’t update")
                                        .kind(MessageDialogKind::Error)
                                        .show(|_| {});
                                }
                            }
                        });
                    });
            }
            Ok(None) => {
                dialog
                    .message(format!(
                        "powermise {} is the newest version.",
                        app.package_info().version
                    ))
                    .title("You’re up to date")
                    .show(|_| {});
            }
            Err(error) => {
                dialog
                    .message(format!("powermise couldn’t check for updates: {error}"))
                    .title("Check for Updates")
                    .kind(MessageDialogKind::Warning)
                    .show(|_| {});
            }
        }
    });
}
