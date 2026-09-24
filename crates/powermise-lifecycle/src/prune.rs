//! `prune`: remove ledger-owned generated artifacts whose selection is no
//! longer managed, and report unmanaged configuration files that are skipped.
use std::path::{Path, PathBuf};

use powermise_domain::{OperationProvider, ResourceKind};
use powermise_state::{Ledger, load_ledger, write_ledger};

use crate::{LifecycleError, PowermiseLayout, SelectionStore, load_manifest};

/// A single prune action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PruneAction {
    /// A ledger-owned artifact was (or would be) removed.
    Remove { path: PathBuf, reason: String },
    /// An unmanaged generated-looking file was found but never removed.
    SkipUnmanaged { path: PathBuf },
}

/// The outcome of a prune run.
pub struct PruneReport {
    pub actions: Vec<PruneAction>,
    pub dry_run: bool,
    pub ledger_updated: bool,
}

impl PruneReport {
    pub fn removed(&self) -> usize {
        self.actions
            .iter()
            .filter(|action| matches!(action, PruneAction::Remove { .. }))
            .count()
    }

    pub fn skipped_unmanaged(&self) -> usize {
        self.actions
            .iter()
            .filter(|action| matches!(action, PruneAction::SkipUnmanaged { .. }))
            .count()
    }
}

/// Removes stale ledger-owned generated configurations, manifests, and stub
/// scripts. In dry-run mode nothing is written or removed. Unmanaged
/// `config.powermise-*.toml` files are reported and never touched.
pub fn prune(
    layout: &PowermiseLayout,
    mise_config_dir: &Path,
    dry_run: bool,
) -> Result<PruneReport, LifecycleError> {
    let mut actions = Vec::new();
    let mut ledger = load_ledger(&layout.ledger_path)?;
    let managed_names: Vec<String> = SelectionStore::new(layout).list()?;
    let generated_entries = ledger
        .resources
        .values()
        .filter(|record| {
            record.provider == OperationProvider::Generated
                && matches!(
                    record.resource_kind,
                    ResourceKind::GeneratedFile | ResourceKind::StubScript
                )
        })
        .cloned()
        .collect::<Vec<_>>();

    for record in &generated_entries {
        let stale = !managed_names.contains(&record.selection);
        if !stale {
            continue;
        }
        let selection = record.selection.clone();
        let mut owned_paths = Vec::new();
        match record.resource_kind {
            ResourceKind::StubScript => {
                let path = PathBuf::from(&record.resource_id);
                owned_paths.push(path.clone());
                actions.push(PruneAction::Remove {
                    path,
                    reason: format!(
                        "selection `{selection}` is no longer managed but owns a stub script"
                    ),
                });
            }
            _ => {
                if let Some(manifest) = load_manifest(layout, &selection)? {
                    let config_path = mise_config_dir.join(&manifest.generated_config_file);
                    owned_paths.push(config_path);
                    actions.push(PruneAction::Remove {
                        path: owned_paths[owned_paths.len() - 1].clone(),
                        reason: format!(
                            "selection `{selection}` is no longer managed but owns a generated configuration"
                        ),
                    });
                }
                let manifest_path = layout.manifest_path(&selection);
                if manifest_path.exists() {
                    owned_paths.push(manifest_path);
                    actions.push(PruneAction::Remove {
                        path: owned_paths[owned_paths.len() - 1].clone(),
                        reason: format!("selection `{selection}` is no longer managed"),
                    });
                }
            }
        }
        if !dry_run {
            for path in &owned_paths {
                if let Err(source) = std::fs::remove_file(path) {
                    if source.kind() == std::io::ErrorKind::NotFound {
                        continue;
                    }
                    return Err(LifecycleError::Io {
                        path: path.clone(),
                        source,
                    });
                }
            }
            ledger.remove(&record.operation_id);
        }
    }

    for file in list_powermise_configs(mise_config_dir)? {
        if !ledger
            .resources
            .values()
            .any(|record| mise_config_dir.join(&record.resource_id) == file)
        {
            actions.push(PruneAction::SkipUnmanaged { path: file });
        }
    }

    let ledger_updated = if dry_run {
        false
    } else {
        write_ledger(&layout.ledger_path, &ledger)? != powermise_state::AtomicWriteResult::Unchanged
    };

    Ok(PruneReport {
        actions,
        dry_run,
        ledger_updated,
    })
}

fn list_powermise_configs(mise_config_dir: &Path) -> Result<Vec<PathBuf>, LifecycleError> {
    let entries = match std::fs::read_dir(mise_config_dir) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(LifecycleError::Io {
                path: mise_config_dir.to_path_buf(),
                source,
            });
        }
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| LifecycleError::Io {
            path: mise_config_dir.to_path_buf(),
            source,
        })?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("config.powermise-")
            && name.to_string_lossy().ends_with(".toml")
        {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

/// Loads the ledger, used by uninstall and tests.
pub fn load_lifecycle_ledger(layout: &PowermiseLayout) -> Result<Ledger, LifecycleError> {
    Ok(load_ledger(&layout.ledger_path)?)
}
