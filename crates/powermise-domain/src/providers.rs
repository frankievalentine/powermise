use std::{error::Error, path::PathBuf};

use crate::OperationProvider;

/// A request to install a package through a Powermise-native installer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeInstallRequest {
    pub provider: OperationProvider,
    pub package: String,
    pub version: String,
}

/// The outcome of a non-mutating presence check for a native package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCheckOutput {
    pub present: bool,
    pub detail: String,
}

/// The outcome of a native plan or apply for a single package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeApplyOutput {
    pub changed: bool,
    pub message: String,
}

/// A port for Powermise-owned native package installation.
pub trait NativeInstaller {
    type Error: Error + Send + Sync + 'static;

    /// Reports whether the package is already present.
    fn check(&self, request: &NativeInstallRequest) -> Result<NativeCheckOutput, Self::Error>;

    /// Reports the exact command that an apply would run, without mutating state.
    fn plan(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error>;

    /// Installs the package and verifies the result.
    fn apply(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error>;
}

/// The managed content to inject into a configuration document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedBlock {
    /// A selection-scoped marker that identifies the Powermise-owned block.
    pub marker: String,
    /// The text to inject between the managed-block markers.
    pub contents: String,
}

/// A request to reconcile a managed configuration block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigAdapterRequest {
    /// The application whose configuration is mutated, e.g. `claude`.
    pub config_target: String,
    /// The owning operation ID, e.g. `config-adapter:integration:claude-github`.
    pub resource_id: String,
    /// The configuration file to reconcile.
    pub path: PathBuf,
    pub managed_block: ManagedBlock,
    /// The file fingerprint recorded by the last apply, used to detect drift.
    pub applied_fingerprint: Option<String>,
}

/// The observed state of a managed configuration block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigAdapterState {
    /// The managed block is absent and must be written.
    Missing,
    /// The managed block is present and matches its last applied fingerprint.
    Current,
    /// The file changed since the last apply and must be inspected before writing.
    Drifted,
}

/// The outcome of a config-adapter check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigAdapterCheckOutput {
    pub state: ConfigAdapterState,
    pub detail: String,
}

/// The outcome of a config-adapter plan or apply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigAdapterApplyOutput {
    pub changed: bool,
    pub before_fingerprint: Option<String>,
    pub after_fingerprint: String,
}

/// A port for typed configuration adapters with snapshots and reversible lifecycle.
pub trait ConfigMutator {
    type Error: Error + Send + Sync + 'static;

    /// Reports whether the managed block is present, current, or drifted.
    fn check(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterCheckOutput, Self::Error>;

    /// Computes the would-be apply outcome without mutating the file.
    fn plan(&self, request: &ConfigAdapterRequest)
    -> Result<ConfigAdapterApplyOutput, Self::Error>;

    /// Reconciles the managed block and returns before/after fingerprints.
    fn apply(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterApplyOutput, Self::Error>;

    /// Removes the managed block, restoring the file to its prior state.
    fn restore(
        &self,
        request: &ConfigAdapterRequest,
    ) -> Result<ConfigAdapterApplyOutput, Self::Error>;
}
