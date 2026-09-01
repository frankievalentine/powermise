//! `uninstall`: report and remove a selection's ledger-owned generated
//! configuration, manifest, stub scripts, and ledger entries.
use std::path::{Path, PathBuf};

use powermise_domain::{OperationProvider, ResourceKind};

use crate::{LifecycleError, PowermiseLayout, load_manifest};

/// A single uninstall action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallAction {
    pub path: PathBuf,
    pub reason: String,
}

/// The outcome of an uninstall run.
pub struct UninstallReport {
    pub selection: String,
    pub actions: Vec<UninstallAction>,
    pub dry_run: bool,
    pub ledger_updated: bool,
}

/// Removes the ledger-owned generated artifacts for `name`. In dry-run mode
/// nothing is removed or written.
pub fn uninstall(
    layout: &PowermiseLayout,
    name: &str,
    mise_config_dir: &Path,
    dry_run: bool,
) -> Result<UninstallReport, LifecycleError> {
    let mut ledger = crate::prune::load_lifecycle_ledger(layout)?;
    let owned = ledger
        .resources
        .values()
        .filter(|record| {
            record.selection == name
                && record.provider == OperationProvider::Generated
                && matches!(
                    record.resource_kind,
                    ResourceKind::GeneratedFile | ResourceKind::StubScript
                )
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut actions = Vec::new();
    for record in &owned {
        match record.resource_kind {
            ResourceKind::StubScript => {
                let path = PathBuf::from(&record.resource_id);
                actions.push(UninstallAction {
                    path: path.clone(),
                    reason: format!("stub script for `{name}`"),
                });
                if !dry_run {
                    remove_ignoring_missing(&path)?;
                    ledger.remove(&record.operation_id);
                }
            }
            _ => {
                if let Some(manifest) = load_manifest(layout, name)? {
                    let config_path = mise_config_dir.join(&manifest.generated_config_file);
                    actions.push(UninstallAction {
                        path: config_path,
                        reason: format!("generated configuration for `{name}`"),
                    });
                }
                let manifest_path = layout.manifest_path(name);
                if manifest_path.exists() {
                    actions.push(UninstallAction {
                        path: manifest_path,
                        reason: format!("manifest for `{name}`"),
                    });
                }
                if !dry_run {
                    for action in &actions {
                        remove_ignoring_missing(&action.path)?;
                    }
                    ledger.remove(&record.operation_id);
                }
            }
        }
    }

    let ledger_updated = if dry_run {
        false
    } else {
        powermise_state::write_ledger(&layout.ledger_path, &ledger)?
            != powermise_state::AtomicWriteResult::Unchanged
    };

    Ok(UninstallReport {
        selection: name.into(),
        actions,
        dry_run,
        ledger_updated,
    })
}

fn remove_ignoring_missing(path: &Path) -> Result<(), LifecycleError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(LifecycleError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}
