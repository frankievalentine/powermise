//! Powermise selection lifecycle: managed selection storage, activation, and
//! safety operations (diff, doctor, prune, uninstall) over ledger-owned
//! resources, plus the convergent apply command.
mod activate;
mod apply;
mod diff;
mod doctor;
mod exec;
mod prune;
mod selection;
mod shell;
mod status;
mod stub;
#[cfg(test)]
mod tests;
mod uninstall;

pub use activate::{ActivationOutput, activate};
pub use apply::{ApplyOptions, ApplyReport, BootstrapOutcome, apply};
pub use diff::{DiffOutput, diff};
pub use doctor::{Finding, FindingSeverity, doctor, has_errors};
pub use exec::{ExecReport, exec};
pub use prune::{PruneAction, PruneReport, prune};
pub use selection::SelectionStore;
pub use shell::{
    Shell, ShellInstallReport, ShellUninstallReport, install_shell_integration,
    uninstall_shell_integration,
};
pub use status::{NativeStatus, StatusReport, status};
pub use stub::{StubInstallReport, StubRemoveReport, install_stubs, remove_stubs};
pub use uninstall::{UninstallAction, UninstallReport, uninstall};

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use powermise_compiler::{compile_mise, compile_plan};
use powermise_domain::{
    Operation, OperationAction, OperationOwnership, OperationProvider, OperationReversibility,
    OperationVerification, Platform, ResourceKind, ReversibilityKind, Selection, VerificationKind,
};
use powermise_registry::{
    ResolvedSelection, load_app_catalog, load_catalog, load_integration_catalog, load_mcp_catalog,
    load_selection, load_tool_catalog, resolve_selection, validate_catalog, validate_registry,
    validate_selection, validate_selection_app_references,
};
use powermise_state::{
    GeneratedConfigArtifact, GeneratedManifest, Ledger, StateError, inspect_generated_file,
    load_ledger, write_ledger,
};
use thiserror::Error;

/// Builds the mise bootstrap-status request for a compiled context.
pub(crate) fn mise_request(context: &CompilationContext) -> powermise_domain::MiseDryRunRequest {
    powermise_domain::MiseDryRunRequest {
        environment: context.artifact.manifest().environment.clone(),
        config_file_name: context.artifact.manifest().generated_config_file.clone(),
        config_contents: context.artifact.contents().to_owned(),
    }
}

/// The default catalog paths used by the CLI and lifecycle commands.
pub struct CatalogPaths {
    pub harnesses: PathBuf,
    pub tools: PathBuf,
    pub apps: PathBuf,
    pub integrations: PathBuf,
    pub mcp: PathBuf,
}

/// The compiled context shared by plan, activate, diff, doctor, and apply.
pub struct CompilationContext {
    pub resolved: ResolvedSelection,
    pub artifact: GeneratedConfigArtifact,
    pub plan: powermise_domain::Plan,
}

/// The Powermise-managed directory layout.
pub struct PowermiseLayout {
    /// The Powermise root directory, for example `~/.config/powermise`.
    pub root: PathBuf,
    /// Managed selection files: `<root>/selections/<name>.toml`.
    pub selections: PathBuf,
    /// State directory: `<root>/state`.
    pub state: PathBuf,
    /// Persisted manifests: `<root>/state/manifests`.
    pub manifests: PathBuf,
    /// Persisted run reports: `<root>/state/run-reports`.
    pub run_reports: PathBuf,
    /// The ownership ledger: `<root>/state/ledger.json`.
    pub ledger_path: PathBuf,
}

impl PowermiseLayout {
    /// Resolves the Powermise directory from an explicit override, then
    /// `POWERMISE_HOME`, `XDG_CONFIG_HOME/powermise`, or `~/.config/powermise`.
    pub fn resolve(override_dir: Option<PathBuf>) -> Result<Self, LifecycleError> {
        let root = match override_dir {
            Some(path) => path,
            None => env::var_os("POWERMISE_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    env::var_os("XDG_CONFIG_HOME").map(|path| PathBuf::from(path).join("powermise"))
                })
                .or_else(|| {
                    env::var_os("HOME").map(|path| PathBuf::from(path).join(".config/powermise"))
                })
                .ok_or(LifecycleError::NoConfigDir)?,
        };
        Ok(Self {
            selections: root.join("selections"),
            state: root.join("state"),
            manifests: root.join("state/manifests"),
            run_reports: root.join("state/run-reports"),
            ledger_path: root.join("state/ledger.json"),
            root,
        })
    }

    pub fn selection_path(&self, name: &str) -> PathBuf {
        self.selections.join(format!("{name}.toml"))
    }

    pub fn manifest_path(&self, name: &str) -> PathBuf {
        self.manifests.join(format!("{name}.manifest.json"))
    }

    pub fn run_report_path(&self, name: &str) -> PathBuf {
        self.run_reports.join(format!("{name}.json"))
    }

    fn ensure_directories(&self) -> Result<(), LifecycleError> {
        for directory in [&self.selections, &self.manifests, &self.run_reports] {
            fs::create_dir_all(directory).map_err(|source| LifecycleError::Io {
                path: directory.clone(),
                source,
            })?;
        }
        Ok(())
    }
}

/// Loads and validates all catalogs, then compiles the resolved selection and plan.
pub fn compile_context(
    paths: &CatalogPaths,
    selection_path: &Path,
    target: Platform,
    machine_role: Option<&str>,
) -> Result<CompilationContext, LifecycleError> {
    let catalog = load_catalog(&paths.harnesses)?;
    validate_catalog(&catalog)?;
    let tools = load_tool_catalog(&paths.tools)?;
    let apps = load_app_catalog(&paths.apps)?;
    let integrations = load_integration_catalog(&paths.integrations)?;
    let mcp = load_mcp_catalog(&paths.mcp)?;
    validate_registry(&catalog, &tools, &integrations, &mcp, &apps)?;
    let selection = load_selection(selection_path)?;
    validate_selection(&selection, &catalog, target, machine_role)?;
    validate_selection_app_references(&selection, &apps)?;
    let resolved = resolve_selection(
        &selection,
        &catalog,
        &tools,
        &integrations,
        &mcp,
        target,
        machine_role,
    )?;
    let config = compile_mise(&resolved, &tools, &apps, target)?;
    let plan = compile_plan(
        &selection.name,
        &resolved,
        &tools,
        &apps,
        &integrations,
        &mcp,
        target,
    )?;
    let compiled_toml = config.to_toml()?;
    let selection_source = read_file(selection_path)?;
    let catalog_source = read_file(&paths.harnesses)?;
    let tools_source = read_file(&paths.tools)?;
    let apps_source = read_file(&paths.apps)?;
    let integrations_source = read_file(&paths.integrations)?;
    let mcp_source = read_file(&paths.mcp)?;
    let artifact = GeneratedConfigArtifact::new(
        &selection.name,
        &compiled_toml,
        &selection_source,
        &[
            catalog_source.as_slice(),
            tools_source.as_slice(),
            apps_source.as_slice(),
            integrations_source.as_slice(),
            mcp_source.as_slice(),
        ],
    )?;

    Ok(CompilationContext {
        resolved,
        artifact,
        plan,
    })
}

/// Loads a managed selection file by name.
pub fn load_managed_selection(
    layout: &PowermiseLayout,
    name: &str,
) -> Result<Selection, LifecycleError> {
    let path = layout.selection_path(name);
    if !path.exists() {
        return Err(LifecycleError::NotFound { name: name.into() });
    }
    load_selection(&path).map_err(Into::into)
}

fn read_file(path: &Path) -> Result<Vec<u8>, LifecycleError> {
    fs::read(path).map_err(|source| LifecycleError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Validates a selection name for use in file names, environment names, and
/// managed markers (letters, digits, `.`, `_`, `-`).
pub(crate) fn validate_selection_name(selection: &str) -> Result<(), LifecycleError> {
    let valid = !selection.is_empty()
        && selection
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character));
    if valid {
        Ok(())
    } else {
        Err(LifecycleError::Other {
            message: format!(
                "selection name `{selection}` is not valid; \
                 use letters, digits, `.`, `_`, or `-`"
            ),
        })
    }
}

/// Inspects the generated configuration change for a compiled context without
/// writing anything.
pub fn inspect_config(
    context: &CompilationContext,
    name: &str,
    mise_config_dir: &Path,
) -> Result<powermise_state::GeneratedFileChange, LifecycleError> {
    let _ = name;
    let config_path = context.artifact.config_path(mise_config_dir);
    Ok(inspect_generated_file(
        &config_path,
        context.artifact.contents().as_bytes(),
    )?)
}

/// Builds the ledger ownership operation for a selection's generated configuration.
pub fn generated_config_operation(
    selection: &str,
    artifact: &GeneratedConfigArtifact,
) -> Operation {
    Operation {
        id: format!("generated-config:{selection}"),
        provider: OperationProvider::Generated,
        resource_kind: ResourceKind::GeneratedFile,
        resource_id: artifact.manifest().generated_config_file.clone(),
        action: OperationAction::Write,
        requested_state: artifact.manifest().content_sha256.clone(),
        depends_on: Vec::new(),
        ownership: OperationOwnership {
            owner: "powermise".into(),
            selection: selection.into(),
        },
        verification: OperationVerification {
            kind: VerificationKind::FileFingerprint,
            expected: artifact.manifest().content_sha256.clone(),
        },
        reversibility: OperationReversibility {
            kind: ReversibilityKind::DeclarationOnly,
            note: "remove the generated configuration and its manifest".into(),
        },
    }
}

/// Records a generated-config ownership entry and persists the ledger.
pub fn record_generated_ownership(
    layout: &PowermiseLayout,
    selection: &str,
    artifact: &GeneratedConfigArtifact,
    mise_config_dir: &Path,
) -> Result<bool, LifecycleError> {
    let config_path = artifact.config_path(mise_config_dir);
    let observed_fingerprint = fs::read(&config_path)
        .ok()
        .map(|bytes| powermise_state::content_fingerprint(&bytes));
    let mut ledger = load_ledger(&layout.ledger_path)?;
    ledger.record_verified(
        &generated_config_operation(selection, artifact),
        observed_fingerprint,
    )?;
    write_ledger(&layout.ledger_path, &ledger)?;
    Ok(true)
}

/// Loads the persisted manifest for a selection, if present.
pub fn load_manifest(
    layout: &PowermiseLayout,
    selection: &str,
) -> Result<Option<GeneratedManifest>, LifecycleError> {
    let path = layout.manifest_path(selection);
    let contents = match fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(LifecycleError::Io { path, source });
        }
    };
    let manifest: GeneratedManifest = serde_json::from_slice(&contents).map_err(|source| {
        LifecycleError::State(StateError::Json {
            operation: "parse generated manifest",
            path,
            source,
        })
    })?;
    Ok(Some(manifest))
}

/// Removes an owned generated configuration and its manifest, if present.
pub fn remove_generated_artifacts(
    layout: &PowermiseLayout,
    selection: &str,
    mise_config_dir: &Path,
) -> Result<(), LifecycleError> {
    if let Some(manifest) = load_manifest(layout, selection)? {
        let config_path = mise_config_dir.join(&manifest.generated_config_file);
        match fs::remove_file(&config_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(LifecycleError::Io {
                    path: config_path,
                    source,
                });
            }
        }
    }
    let manifest_path = layout.manifest_path(selection);
    match fs::remove_file(&manifest_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(LifecycleError::Io {
                path: manifest_path,
                source,
            });
        }
    }
    let mut ledger: Ledger = load_ledger(&layout.ledger_path)?;
    ledger.remove(&format!("generated-config:{selection}"));
    write_ledger(&layout.ledger_path, &ledger)?;
    Ok(())
}

/// A lifecycle error.
#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("could not load or validate the registry: {0}")]
    Registry(#[from] powermise_registry::RegistryError),
    #[error("could not compile the selection: {0}")]
    Compiler(#[from] powermise_compiler::CompilerError),
    #[error("state error: {0}")]
    State(#[from] StateError),
    #[error("mise error: {0}")]
    Mise(#[from] powermise_mise::MiseAdapterError),
    #[error("could not read or write `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not parse `{path}`: {source}")]
    Toml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("could not serialize `{path}`: {source}")]
    TomlSerialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
    #[error("could not determine the Powermise configuration directory")]
    NoConfigDir,
    #[error("selection `{name}` does not exist")]
    NotFound { name: String },
    #[error("selection `{name}` already exists at `{path}`")]
    AlreadyExists { name: String, path: PathBuf },
    #[error("selection `{name}` is not installed; run `powermise selection activate` first")]
    NotActivated { name: String },
    #[error("no config adapter is available for target `{target}`")]
    UnsupportedConfigTarget { target: String },
    #[error("{message}")]
    Other { message: String },
}

impl From<toml::de::Error> for LifecycleError {
    fn from(source: toml::de::Error) -> Self {
        Self::Toml {
            path: PathBuf::from("<selection>"),
            source,
        }
    }
}
