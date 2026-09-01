use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Platform;

pub const PLAN_SCHEMA_VERSION: u32 = 1;
pub const RUN_REPORT_SCHEMA_VERSION: u32 = 1;
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationProvider {
    MiseTools,
    MiseBootstrap,
    Winget,
    Scoop,
    Npm,
    VerifiedScript,
    ConfigAdapter,
    Generated,
}

impl std::fmt::Display for OperationProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MiseTools => formatter.write_str("mise-tools"),
            Self::MiseBootstrap => formatter.write_str("mise-bootstrap"),
            Self::Winget => formatter.write_str("winget"),
            Self::Scoop => formatter.write_str("scoop"),
            Self::Npm => formatter.write_str("npm"),
            Self::VerifiedScript => formatter.write_str("verified-script"),
            Self::ConfigAdapter => formatter.write_str("config-adapter"),
            Self::Generated => formatter.write_str("generated"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    Tool,
    Package,
    Application,
    Repository,
    Dotfile,
    SystemSetting,
    UserService,
    Integration,
    McpServer,
    GeneratedFile,
    StubScript,
    Configuration,
}

impl std::fmt::Display for ResourceKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tool => formatter.write_str("tool"),
            Self::Package => formatter.write_str("package"),
            Self::Application => formatter.write_str("application"),
            Self::Repository => formatter.write_str("repository"),
            Self::Dotfile => formatter.write_str("dotfile"),
            Self::SystemSetting => formatter.write_str("system-setting"),
            Self::UserService => formatter.write_str("user-service"),
            Self::Integration => formatter.write_str("integration"),
            Self::McpServer => formatter.write_str("mcp-server"),
            Self::GeneratedFile => formatter.write_str("generated-file"),
            Self::StubScript => formatter.write_str("stub-script"),
            Self::Configuration => formatter.write_str("configuration"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationAction {
    EnsurePresent,
    Write,
    Remove,
    Restore,
    Configure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationOwnership {
    pub owner: String,
    pub selection: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationKind {
    MiseStatus,
    Command,
    FileFingerprint,
    AdapterCheck,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationVerification {
    pub kind: VerificationKind,
    pub expected: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReversibilityKind {
    DeclarationOnly,
    RestorableSnapshot,
    NotAutomated,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationReversibility {
    pub kind: ReversibilityKind,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Operation {
    pub id: String,
    pub provider: OperationProvider,
    pub resource_kind: ResourceKind,
    pub resource_id: String,
    pub action: OperationAction,
    pub requested_state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    pub ownership: OperationOwnership,
    pub verification: OperationVerification,
    pub reversibility: OperationReversibility,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationGraph {
    operations: Vec<Operation>,
}

impl OperationGraph {
    pub fn try_new(mut operations: Vec<Operation>) -> Result<Self, OperationGraphError> {
        for operation in &mut operations {
            operation.depends_on.sort();
            operation.depends_on.dedup();
        }

        let mut operations_by_id = BTreeMap::new();
        for operation in operations {
            let operation_id = operation.id.clone();
            if operations_by_id
                .insert(operation_id.clone(), operation)
                .is_some()
            {
                return Err(OperationGraphError::DuplicateOperation { operation_id });
            }
        }

        let mut dependency_counts = BTreeMap::new();
        let mut dependents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (operation_id, operation) in &operations_by_id {
            dependency_counts.insert(operation_id.clone(), operation.depends_on.len());
            for dependency_id in &operation.depends_on {
                if !operations_by_id.contains_key(dependency_id) {
                    return Err(OperationGraphError::MissingDependency {
                        operation_id: operation_id.clone(),
                        dependency_id: dependency_id.clone(),
                    });
                }
                dependents
                    .entry(dependency_id.clone())
                    .or_default()
                    .insert(operation_id.clone());
            }
        }

        let mut ready = dependency_counts
            .iter()
            .filter_map(|(operation_id, count)| (*count == 0).then_some(operation_id.clone()))
            .collect::<BTreeSet<_>>();
        let mut ordered = Vec::with_capacity(operations_by_id.len());

        while let Some(operation_id) = ready.pop_first() {
            let operation = operations_by_id
                .remove(&operation_id)
                .expect("ready operations must exist in the graph");
            ordered.push(operation);

            if let Some(operation_dependents) = dependents.get(&operation_id) {
                for dependent_id in operation_dependents {
                    let count = dependency_counts
                        .get_mut(dependent_id)
                        .expect("dependent operations must have a dependency count");
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(dependent_id.clone());
                    }
                }
            }
        }

        if !operations_by_id.is_empty() {
            return Err(OperationGraphError::Cycle {
                operation_ids: operations_by_id.into_keys().collect(),
            });
        }

        Ok(Self {
            operations: ordered,
        })
    }

    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    pub fn into_operations(self) -> Vec<Operation> {
        self.operations
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Plan {
    pub schema_version: u32,
    pub selection: String,
    pub platform: Platform,
    pub operations: OperationGraph,
}

impl Plan {
    pub fn new(
        selection: impl Into<String>,
        platform: Platform,
        operations: OperationGraph,
    ) -> Self {
        Self {
            schema_version: PLAN_SCHEMA_VERSION,
            selection: selection.into(),
            platform,
            operations,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationOutcome {
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationResult {
    pub operation_id: String,
    pub outcome: OperationOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunReport {
    pub schema_version: u32,
    pub selection: String,
    pub platform: Platform,
    pub operations: Vec<OperationResult>,
}

impl RunReport {
    pub fn new(
        selection: impl Into<String>,
        platform: Platform,
        operations: Vec<OperationResult>,
    ) -> Self {
        Self {
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            selection: selection.into(),
            platform,
            operations,
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum OperationGraphError {
    #[error("operation graph contains duplicate operation `{operation_id}`")]
    DuplicateOperation { operation_id: String },
    #[error("operation `{operation_id}` depends on missing operation `{dependency_id}`")]
    MissingDependency {
        operation_id: String,
        dependency_id: String,
    },
    #[error("operation graph contains a dependency cycle involving: {operation_ids:?}")]
    Cycle { operation_ids: Vec<String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(id: &str, depends_on: &[&str]) -> Operation {
        Operation {
            id: id.into(),
            provider: OperationProvider::MiseTools,
            resource_kind: ResourceKind::Tool,
            resource_id: id.into(),
            action: OperationAction::EnsurePresent,
            requested_state: "latest".into(),
            depends_on: depends_on
                .iter()
                .map(|dependency| (*dependency).into())
                .collect(),
            ownership: OperationOwnership {
                owner: "powermise".into(),
                selection: "test".into(),
            },
            verification: OperationVerification {
                kind: VerificationKind::MiseStatus,
                expected: "present".into(),
            },
            reversibility: OperationReversibility {
                kind: ReversibilityKind::DeclarationOnly,
                note: "remove the generated declaration".into(),
            },
        }
    }

    #[test]
    fn graph_orders_ready_operations_deterministically() {
        let graph = OperationGraph::try_new(vec![
            operation("c", &[]),
            operation("b", &["a"]),
            operation("a", &[]),
        ])
        .expect("graph should be valid");

        let ids = graph
            .operations()
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["a", "b", "c"]);
    }

    #[test]
    fn graph_deduplicates_dependency_edges() {
        let graph = OperationGraph::try_new(vec![operation("a", &[]), operation("b", &["a", "a"])])
            .expect("graph should be valid");

        assert_eq!(graph.operations()[1].depends_on, ["a"]);
    }

    #[test]
    fn graph_rejects_duplicate_operation_ids() {
        let error = OperationGraph::try_new(vec![operation("a", &[]), operation("a", &[])])
            .expect_err("duplicate IDs must fail");

        assert_eq!(
            error,
            OperationGraphError::DuplicateOperation {
                operation_id: "a".into()
            }
        );
    }

    #[test]
    fn graph_rejects_missing_dependencies() {
        let error = OperationGraph::try_new(vec![operation("a", &["missing"])])
            .expect_err("missing dependencies must fail");

        assert_eq!(
            error,
            OperationGraphError::MissingDependency {
                operation_id: "a".into(),
                dependency_id: "missing".into(),
            }
        );
    }

    #[test]
    fn graph_rejects_dependency_cycles() {
        let error = OperationGraph::try_new(vec![operation("a", &["b"]), operation("b", &["a"])])
            .expect_err("cycles must fail");

        assert_eq!(
            error,
            OperationGraphError::Cycle {
                operation_ids: vec!["a".into(), "b".into()],
            }
        );
    }
}
