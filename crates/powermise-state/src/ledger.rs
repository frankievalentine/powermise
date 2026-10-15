use std::{collections::BTreeMap, fs, path::Path};

use powermise_domain::{Operation, OperationProvider, ResourceKind};
use serde::{Deserialize, Serialize};

use crate::{AtomicWriteResult, StateError, write_atomically};

pub const LEDGER_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnershipRecord {
    pub operation_id: String,
    pub selection: String,
    pub provider: OperationProvider,
    pub resource_kind: ResourceKind,
    pub resource_id: String,
    pub requested_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Ledger {
    pub schema_version: u32,
    pub resources: BTreeMap<String, OwnershipRecord>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            schema_version: LEDGER_SCHEMA_VERSION,
            resources: BTreeMap::new(),
        }
    }
}

impl Ledger {
    pub fn record_verified(
        &mut self,
        operation: &Operation,
        observed_fingerprint: Option<String>,
    ) -> Result<(), StateError> {
        if operation.ownership.owner != "powermise" {
            return Err(StateError::UnownedOperation {
                operation_id: operation.id.clone(),
                owner: operation.ownership.owner.clone(),
            });
        }

        self.resources.insert(
            operation.id.clone(),
            OwnershipRecord {
                operation_id: operation.id.clone(),
                selection: operation.ownership.selection.clone(),
                provider: operation.provider,
                resource_kind: operation.resource_kind,
                resource_id: operation.resource_id.clone(),
                requested_state: operation.requested_state.clone(),
                observed_fingerprint,
            },
        );
        Ok(())
    }

    pub fn remove(&mut self, operation_id: &str) -> Option<OwnershipRecord> {
        self.resources.remove(operation_id)
    }
}

pub fn load_ledger(path: &Path) -> Result<Ledger, StateError> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Ledger::default()),
        Err(source) => {
            return Err(StateError::Io {
                operation: "read ownership ledger",
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let ledger: Ledger = serde_json::from_slice(&contents).map_err(|source| StateError::Json {
        operation: "parse ownership ledger",
        path: path.to_path_buf(),
        source,
    })?;

    if ledger.schema_version != LEDGER_SCHEMA_VERSION {
        return Err(StateError::UnsupportedSchema {
            kind: "ownership ledger",
            version: ledger.schema_version,
            expected: LEDGER_SCHEMA_VERSION,
        });
    }

    Ok(ledger)
}

pub fn write_ledger(path: &Path, ledger: &Ledger) -> Result<AtomicWriteResult, StateError> {
    if ledger.schema_version != LEDGER_SCHEMA_VERSION {
        return Err(StateError::UnsupportedSchema {
            kind: "ownership ledger",
            version: ledger.schema_version,
            expected: LEDGER_SCHEMA_VERSION,
        });
    }

    let mut contents = serde_json::to_vec_pretty(ledger).map_err(|source| StateError::Json {
        operation: "serialize ownership ledger",
        path: path.to_path_buf(),
        source,
    })?;
    contents.push(b'\n');
    write_atomically(path, &contents)
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use powermise_domain::{
        OperationAction, OperationOwnership, OperationReversibility, OperationVerification,
        ReversibilityKind, VerificationKind,
    };

    use super::*;

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_file() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "powermise-ledger-test-{}-{timestamp}-{sequence}",
                process::id(),
            ))
            .join("state/ledger.json")
    }

    fn operation(owner: &str) -> Operation {
        Operation {
            id: "mise-tool:node".into(),
            provider: OperationProvider::MiseTools,
            resource_kind: ResourceKind::Tool,
            resource_id: "node".into(),
            action: OperationAction::EnsurePresent,
            requested_state: "24".into(),
            depends_on: Vec::new(),
            ownership: OperationOwnership {
                owner: owner.into(),
                selection: "engineering-agents".into(),
            },
            verification: OperationVerification {
                kind: VerificationKind::MiseStatus,
                expected: "node@24".into(),
            },
            reversibility: OperationReversibility {
                kind: ReversibilityKind::DeclarationOnly,
                note: "declaration only".into(),
            },
        }
    }

    #[test]
    fn missing_ledger_loads_as_empty_current_schema() {
        let ledger = load_ledger(&test_file()).expect("missing ledger should be empty");

        assert_eq!(ledger, Ledger::default());
    }

    #[test]
    fn verified_records_persist_deterministically_and_converge() {
        let path = test_file();
        let mut ledger = Ledger::default();
        ledger
            .record_verified(&operation("powermise"), Some("fingerprint".into()))
            .expect("powermise operation should be recorded");

        assert_eq!(
            write_ledger(&path, &ledger).expect("ledger should be written"),
            AtomicWriteResult::Created
        );
        assert_eq!(
            write_ledger(&path, &ledger).expect("repeat write should converge"),
            AtomicWriteResult::Unchanged
        );
        assert_eq!(load_ledger(&path).expect("ledger should load"), ledger);

        let _ = fs::remove_dir_all(
            path.parent()
                .and_then(Path::parent)
                .expect("test path should have a root"),
        );
    }

    #[test]
    fn ledger_rejects_non_powermise_ownership() {
        let mut ledger = Ledger::default();
        let error = ledger
            .record_verified(&operation("external"), None)
            .expect_err("external resources must not enter the ledger");

        assert!(matches!(error, StateError::UnownedOperation { owner, .. } if owner == "external"));
        assert!(ledger.resources.is_empty());
    }

    #[test]
    fn ledger_rejects_unsupported_schema_on_write() {
        let path = test_file();
        let ledger = Ledger {
            schema_version: LEDGER_SCHEMA_VERSION + 1,
            resources: BTreeMap::new(),
        };

        assert!(matches!(
            write_ledger(&path, &ledger),
            Err(StateError::UnsupportedSchema { .. })
        ));
        assert!(!path.exists());
    }
}
