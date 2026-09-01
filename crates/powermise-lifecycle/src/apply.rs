//! `apply`: converge a selection's generated configuration, manifest, ledger,
//! and config-adapter managed blocks, and surface mise and native follow-up
//! actions with failure-boundary recovery guidance.
use std::path::Path;

use powermise_domain::{
    ConfigAdapterRequest, ConfigMutator, ManagedBlock, MiseBootstrapApplyRequest, MiseRunner,
    NativeInstallRequest, NativeInstaller, OperationOutcome, OperationProvider, OperationResult,
    Platform, ResourceKind, RunReport,
};
use powermise_providers::{JsonManagedAdapter, NpmProvider, ScoopProvider, WingetProvider};
use powermise_state::GeneratedFileChange;

use crate::{
    CatalogPaths, CompilationContext, LifecycleError, PowermiseLayout, SelectionStore,
    compile_context, inspect_config, mise_request, record_generated_ownership,
};

/// The outcome of an apply run.
pub struct ApplyReport {
    pub selection: String,
    pub change: GeneratedFileChange,
    pub results: Vec<OperationResult>,
    pub run_report_path: std::path::PathBuf,
    /// Whether and how the real `mise bootstrap --yes` convergence ran.
    pub bootstrap: BootstrapOutcome,
    pub mise_follow_ups: Vec<String>,
    pub recovery_guidance: Vec<String>,
    pub ledger_updated: bool,
}

/// Whether and how apply executed the real mise bootstrap convergence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapOutcome {
    /// A real `mise bootstrap --yes` ran and exited successfully.
    Converged,
    /// A real bootstrap ran and failed; nothing was rolled back and the
    /// remaining work is listed in the follow-up actions.
    Failed { message: String },
    /// No real bootstrap ran; mise operations were declared only.
    Skipped { reason: String },
}

/// The directories and catalog paths an apply operates against.
pub struct ApplyOptions<'a> {
    pub layout: &'a PowermiseLayout,
    pub paths: &'a CatalogPaths,
    pub target: Platform,
    pub machine_role: Option<&'a str>,
    pub mise_config_dir: &'a Path,
    pub adapters_config_dir: &'a Path,
    /// Whether apply converges for real by running `mise bootstrap --yes`
    /// against `mise_config_dir` before applying its own operations. When
    /// false, mise-managed resources are declared only and surfaced as
    /// follow-up actions.
    pub run_mise_bootstrap: bool,
    /// An optional progress reporter invoked for human-readable TTY progress.
    /// Ignored in non-interactive and JSON output.
    pub progress: Option<&'a dyn Fn(&str)>,
}

/// Converges the generated configuration, manifest, ledger, and managed config
/// blocks for `name`, then reports mise and native follow-up actions.
pub fn apply<R: MiseRunner>(
    runner: &R,
    options: &ApplyOptions<'_>,
    name: &str,
) -> Result<ApplyReport, LifecycleError>
where
    R::Error: std::error::Error + Send + Sync + 'static,
{
    let layout = options.layout;
    let paths = options.paths;
    let target = options.target;
    let machine_role = options.machine_role;
    let mise_config_dir = options.mise_config_dir;
    let adapters_config_dir = options.adapters_config_dir;
    let report_progress = |message: &str| {
        if let Some(reporter) = options.progress {
            reporter(message);
        }
    };

    SelectionStore::new(layout).read(name)?;
    let context = compile_context(paths, &layout.selection_path(name), target, machine_role)?;
    let change = inspect_config(&context, name, mise_config_dir)?;
    report_progress("writing generated configuration");
    context.artifact.write_config(mise_config_dir)?;
    context.artifact.write_manifest(&layout.manifests)?;
    report_progress("recording ownership ledger");
    let ledger_updated =
        record_generated_ownership(layout, name, &context.artifact, mise_config_dir)?;

    let has_mise_operations = context
        .plan
        .operations
        .operations()
        .iter()
        .any(|operation| {
            matches!(
                operation.provider,
                OperationProvider::MiseTools | OperationProvider::MiseBootstrap
            )
        });

    // BUILD_PLAN Apply steps 3-4: after the generated configuration is in
    // place, converge it for real with `mise bootstrap --yes` unless the
    // caller opted out or the selection owns no mise-managed resources.
    let mut bootstrap_outcome = if !options.run_mise_bootstrap {
        Some(BootstrapOutcome::Skipped {
            reason: "declared only (--skip-bootstrap)".into(),
        })
    } else if !has_mise_operations {
        Some(BootstrapOutcome::Skipped {
            reason: "skipped: the selection owns no mise-managed resources".into(),
        })
    } else {
        None
    };

    let mut results = Vec::new();
    let mut mise_follow_ups = Vec::new();
    let mut recovery_guidance = Vec::new();

    if bootstrap_outcome.is_none() {
        report_progress("converging mise environment (installs tools and packages)");
        let request = MiseBootstrapApplyRequest {
            environment: context.artifact.manifest().environment.clone(),
            config_dir: mise_config_dir.to_path_buf(),
        };
        match runner.bootstrap_apply(&request) {
            Ok(_) => bootstrap_outcome = Some(BootstrapOutcome::Converged),
            Err(error) => {
                recovery_guidance.push(format!(
                    "real `mise bootstrap --yes` failed ({error}); resolve the cause and re-run apply, which is idempotent"
                ));
                bootstrap_outcome = Some(BootstrapOutcome::Failed {
                    message: error.to_string(),
                });
            }
        }
    }

    for operation in context.plan.operations.operations() {
        report_progress(&format!("applying {operation}", operation = operation.id));
        let result = match operation.provider {
            OperationProvider::MiseTools | OperationProvider::MiseBootstrap => {
                let result = match runner.bootstrap_status(&mise_request(&context)) {
                    Ok(status) => {
                        mise_follow_ups.extend(status.follow_up_actions());
                        let message = match (&bootstrap_outcome, status.follow_up_actions().is_empty())
                        {
                            (Some(BootstrapOutcome::Converged), true) => {
                                "converged by real mise bootstrap"
                            }
                            (Some(BootstrapOutcome::Converged), false) => {
                                "mise bootstrap ran; follow-up work remains"
                            }
                            _ => "declared in generated configuration",
                        };
                        operation_result(
                            operation.id.as_str(),
                            OperationOutcome::Succeeded,
                            message,
                        )
                    }
                    Err(error) => {
                        mise_follow_ups.push("mise bootstrap status unavailable".into());
                        operation_result(
                            operation.id.as_str(),
                            OperationOutcome::Succeeded,
                            &format!("declared; mise status unavailable: {error}"),
                        )
                    }
                };
                match &bootstrap_outcome {
                    Some(BootstrapOutcome::Failed { message }) => operation_result(
                        operation.id.as_str(),
                        OperationOutcome::Failed,
                        &format!("real mise bootstrap did not converge: {message}"),
                    ),
                    _ => result,
                }
            }
            OperationProvider::ConfigAdapter => apply_config_adapter(
                &context,
                operation.id.as_str(),
                operation.resource_kind,
                &operation.resource_id,
                &operation.requested_state,
                adapters_config_dir,
            ),
            provider @ (OperationProvider::Winget
            | OperationProvider::Scoop
            | OperationProvider::Npm
            | OperationProvider::VerifiedScript) => apply_native(
                provider,
                operation.id.as_str(),
                &operation.resource_id,
                &operation.requested_state,
            ),
            OperationProvider::Generated => unreachable!("generated ownership is not a plan op"),
        };
        if result.outcome == OperationOutcome::Failed {
            recovery_guidance.push(format!(
                "operation `{}` failed; re-running apply is idempotent, and `powermise diff` shows drift without repairing it",
                result.operation_id
            ));
        }
        results.push(result);
    }

    if !mise_follow_ups.is_empty() {
        mise_follow_ups.sort();
        mise_follow_ups.dedup();
    }

    report_progress("persisting run report");
    let run_report = RunReport::new(name, target, results.clone());
    let run_report_path = layout.run_report_path(name);
    let contents = serde_json::to_vec_pretty(&run_report).map_err(|source| {
        LifecycleError::Other {
            message: format!("could not serialize run report: {source}"),
        }
    })?;
    powermise_state::write_atomically(&run_report_path, &contents)?;

    Ok(ApplyReport {
        selection: name.into(),
        change,
        results,
        run_report_path,
        bootstrap: bootstrap_outcome
            .expect("the bootstrap outcome is decided before operations are applied"),
        mise_follow_ups,
        recovery_guidance,
        ledger_updated,
    })
}

fn operation_result(operation_id: &str, outcome: OperationOutcome, message: &str) -> OperationResult {
    OperationResult {
        operation_id: operation_id.into(),
        outcome,
        message: Some(message.into()),
    }
}

/// Applies a managed JSON block for a config-adapter operation. The block is
/// written under `_powermise.<resource>` at `<config_dir>/<target>/settings.json`,
/// so every config-adapter operation is idempotent and restorable.
fn apply_config_adapter(
    context: &CompilationContext,
    operation_id: &str,
    resource_kind: ResourceKind,
    resource_id: &str,
    config_target: &str,
    adapters_config_dir: &Path,
) -> OperationResult {
    let kind_label = match resource_kind {
        ResourceKind::Integration => "integration",
        ResourceKind::McpServer => "mcp server",
        _ => "resource",
    };
    let contents = serde_json::to_string_pretty(&serde_json::json!({
        "resource": resource_id,
        "config_target": config_target,
        "selection": context.plan.selection,
        "managed_by": "powermise",
    }))
    .unwrap_or_else(|_| "{}".into());
    let request = ConfigAdapterRequest {
        config_target: config_target.into(),
        resource_id: resource_id.into(),
        path: adapters_config_dir
            .join(config_target)
            .join("settings.json"),
        managed_block: ManagedBlock {
            marker: resource_id.into(),
            contents,
        },
        applied_fingerprint: None,
    };
    match JsonManagedAdapter.apply(&request) {
        Ok(output) => operation_result(
            operation_id,
            OperationOutcome::Succeeded,
            &format!(
                "managed {kind_label} `{resource_id}` block at `{}` (changed: {})",
                request.path.display(),
                output.changed
            ),
        ),
        Err(error) => operation_result(
            operation_id,
            OperationOutcome::Failed,
            &format!("could not apply managed {kind_label} `{resource_id}`: {error}"),
        ),
    }
}

/// Executes a native package-manager operation through the corresponding provider.
fn apply_native(
    provider: OperationProvider,
    operation_id: &str,
    package: &str,
    version: &str,
) -> OperationResult {
    let request = NativeInstallRequest {
        provider,
        package: package.into(),
        version: version.into(),
    };
    let result = match provider {
        OperationProvider::Winget => WingetProvider::default().apply(&request),
        OperationProvider::Scoop => ScoopProvider::default().apply(&request),
        OperationProvider::Npm => NpmProvider::default().apply(&request),
        OperationProvider::VerifiedScript => {
            return operation_result(
                operation_id,
                OperationOutcome::Skipped,
                "verified-script operations require a pinned source and SHA-256 not present in the plan",
            );
        }
        _ => unreachable!("apply_native only receives native providers"),
    };
    match result {
        Ok(output) => operation_result(
            operation_id,
            OperationOutcome::Succeeded,
            &format!(
                "native {}: {} (changed: {})",
                package, output.message, output.changed
            ),
        ),
        Err(error) => operation_result(
            operation_id,
            OperationOutcome::Failed,
            &format!("native {} failed: {error}", package),
        ),
    }
}