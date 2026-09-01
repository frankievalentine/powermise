//! Selection activation: compile a managed selection and write its generated
//! configuration, manifest, and ledger ownership entry.
use std::path::Path;

use powermise_domain::Platform;
use powermise_state::GeneratedFileChange;

use crate::{
    CatalogPaths, CompilationContext, LifecycleError, PowermiseLayout, SelectionStore,
    compile_context, record_generated_ownership,
};

/// The outcome of activating a selection.
pub struct ActivationOutput {
    pub change: GeneratedFileChange,
    pub config_path: std::path::PathBuf,
    pub manifest_path: std::path::PathBuf,
    pub ledger_updated: bool,
}

/// Compiles `name` and writes its generated configuration, manifest, and ledger
/// ownership atomically and idempotently.
pub fn activate(
    layout: &PowermiseLayout,
    paths: &CatalogPaths,
    name: &str,
    target: Platform,
    machine_role: Option<&str>,
    mise_config_dir: &Path,
) -> Result<ActivationOutput, LifecycleError> {
    let store = SelectionStore::new(layout);
    store.read(name)?;
    let context = compile_context(paths, &layout.selection_path(name), target, machine_role)?;
    write_activation(&context, name, mise_config_dir, layout)
}

/// Writes the compiled artifact, manifest, and ledger entry for a context.
fn write_activation(
    context: &CompilationContext,
    name: &str,
    mise_config_dir: &Path,
    layout: &PowermiseLayout,
) -> Result<ActivationOutput, LifecycleError> {
    let change = crate::inspect_config(context, name, mise_config_dir)?;
    let config_path = context.artifact.config_path(mise_config_dir);
    context.artifact.write_config(mise_config_dir)?;
    context.artifact.write_manifest(&layout.manifests)?;
    let ledger_updated =
        record_generated_ownership(layout, name, &context.artifact, mise_config_dir)?;
    Ok(ActivationOutput {
        change,
        config_path,
        manifest_path: layout.manifest_path(name),
        ledger_updated,
    })
}