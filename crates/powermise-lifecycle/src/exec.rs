//! `exec`: run a command inside a selection's mise environment.
use std::process::ExitStatus;

use powermise_domain::{MiseRunner, Platform};

use crate::{CatalogPaths, LifecycleError, PowermiseLayout, SelectionStore, compile_context};

/// The outcome of an `exec` run.
pub struct ExecReport {
    pub selection: String,
    pub environment: String,
    pub status: ExitStatus,
}

/// Compiles `name` to resolve its environment, then runs `command` inside that
/// environment with inherited stdio.
pub fn exec<R: MiseRunner>(
    runner: &R,
    layout: &PowermiseLayout,
    paths: &CatalogPaths,
    name: &str,
    target: Platform,
    machine_role: Option<&str>,
    command: &[String],
) -> Result<ExecReport, LifecycleError>
where
    R::Error: std::error::Error + Send + Sync + 'static,
{
    SelectionStore::new(layout).read(name)?;
    let context = compile_context(paths, &layout.selection_path(name), target, machine_role)?;
    let environment = context.artifact.manifest().environment.clone();
    let status = runner.exec(&environment, command).map_err(|error| {
        LifecycleError::Other {
            message: format!("mise exec failed: {error}"),
        }
    })?;
    Ok(ExecReport {
        selection: name.into(),
        environment,
        status,
    })
}