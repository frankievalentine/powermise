//! `status`: combine the generated-config change, manifest drift, ownership
//! ledger state, mise bootstrap follow-ups, and native provider presence for a
//! selection. Never writes.
use std::path::Path;

use powermise_domain::{
    MiseRunner, NativeInstallRequest, NativeInstaller, OperationProvider, Platform,
};
use powermise_providers::{NpmProvider, ScoopProvider, WingetProvider};
use powermise_state::{GeneratedFileChange, load_ledger};

use crate::{
    CatalogPaths, LifecycleError, PowermiseLayout, SelectionStore, compile_context, inspect_config,
    mise_request,
};

/// The observed presence of a single native operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStatus {
    pub operation_id: String,
    pub provider: OperationProvider,
    pub package: String,
    pub present: bool,
    pub detail: String,
}

/// The combined status of a selection.
pub struct StatusReport {
    pub selection: String,
    pub target: Platform,
    pub environment: String,
    pub change: GeneratedFileChange,
    pub drift: Option<String>,
    pub ledger_owned: bool,
    pub mise_follow_ups: Vec<String>,
    pub native: Vec<NativeStatus>,
    pub run_report_exists: bool,
    pub satisfied: bool,
}

/// Compiles `name` and reports desired change, drift, ownership, mise
/// follow-ups, and native presence without writing anything.
pub fn status<R: MiseRunner>(
    runner: &R,
    layout: &PowermiseLayout,
    paths: &CatalogPaths,
    name: &str,
    target: Platform,
    machine_role: Option<&str>,
    mise_config_dir: &Path,
) -> Result<StatusReport, LifecycleError>
where
    R::Error: std::error::Error + Send + Sync + 'static,
{
    SelectionStore::new(layout).read(name)?;
    let context = compile_context(paths, &layout.selection_path(name), target, machine_role)?;
    let environment = context.artifact.manifest().environment.clone();
    let change = inspect_config(&context, name, mise_config_dir)?;
    let drift = crate::diff::detect_drift(layout, &context, name, mise_config_dir)?;

    let ledger = load_ledger(&layout.ledger_path)?;
    let ledger_owned = ledger
        .resources
        .values()
        .any(|record| record.operation_id == format!("generated-config:{name}"));

    let mut mise_follow_ups = Vec::new();
    match runner.bootstrap_status(&mise_request(&context, mise_config_dir)) {
        Ok(observed) => mise_follow_ups.extend(observed.follow_up_actions()),
        Err(error) => {
            mise_follow_ups.push(format!("mise bootstrap status unavailable: {error}"));
        }
    }
    mise_follow_ups.sort();
    mise_follow_ups.dedup();

    let mut native = Vec::new();
    for operation in context.plan.operations.operations() {
        if !matches!(
            operation.provider,
            OperationProvider::Winget
                | OperationProvider::Scoop
                | OperationProvider::Npm
                | OperationProvider::VerifiedScript
        ) {
            continue;
        }
        native.push(check_native(
            operation.id.as_str(),
            operation.provider,
            &operation.resource_id,
        ));
    }

    let run_report_exists = layout.run_report_path(name).exists();
    let satisfied = change.kind == powermise_state::GeneratedFileChangeKind::Unchanged
        && drift.is_none()
        && ledger_owned
        && mise_follow_ups.is_empty()
        && native.iter().all(|entry| entry.present);

    Ok(StatusReport {
        selection: name.into(),
        target,
        environment,
        change,
        drift,
        ledger_owned,
        mise_follow_ups,
        native,
        run_report_exists,
        satisfied,
    })
}

fn check_native(operation_id: &str, provider: OperationProvider, package: &str) -> NativeStatus {
    if provider == OperationProvider::VerifiedScript {
        return NativeStatus {
            operation_id: operation_id.into(),
            provider,
            package: package.into(),
            present: false,
            detail: "verified-script presence requires a pinned source and SHA-256".into(),
        };
    }
    let request = NativeInstallRequest {
        provider,
        package: package.into(),
        version: String::new(),
    };
    let result = match provider {
        OperationProvider::Winget => WingetProvider::default().check(&request),
        OperationProvider::Scoop => ScoopProvider::default().check(&request),
        OperationProvider::Npm => NpmProvider::default().check(&request),
        _ => unreachable!("check_native only receives native providers"),
    };
    match result {
        Ok(output) => NativeStatus {
            operation_id: operation_id.into(),
            provider,
            package: package.into(),
            present: output.present,
            detail: output.detail,
        },
        Err(error) => NativeStatus {
            operation_id: operation_id.into(),
            provider,
            package: package.into(),
            present: false,
            detail: error.to_string(),
        },
    }
}
