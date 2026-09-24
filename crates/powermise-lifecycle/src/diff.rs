//! `diff`: report the desired generated-config change and any drift between the
//! on-disk configuration and the persisted manifest. Never writes.
use std::path::Path;

use powermise_domain::Platform;
use powermise_state::GeneratedFileChange;

use crate::{
    CatalogPaths, CompilationContext, LifecycleError, PowermiseLayout, SelectionStore,
    compile_context, inspect_config, load_manifest,
};

/// The outcome of a `diff` run.
pub struct DiffOutput {
    pub selection: String,
    /// The desired change to the generated configuration.
    pub change: GeneratedFileChange,
    /// Drift description, present when the on-disk configuration no longer
    /// matches the persisted manifest.
    pub drift: Option<String>,
}

/// Compiles `name` and reports the desired change and drift without writing.
pub fn diff(
    layout: &PowermiseLayout,
    paths: &CatalogPaths,
    name: &str,
    target: Platform,
    machine_role: Option<&str>,
    mise_config_dir: &Path,
) -> Result<DiffOutput, LifecycleError> {
    SelectionStore::new(layout).read(name)?;
    let context = compile_context(paths, &layout.selection_path(name), target, machine_role)?;
    let change = inspect_config(&context, name, mise_config_dir)?;
    let drift = detect_drift(layout, &context, name, mise_config_dir)?;
    Ok(DiffOutput {
        selection: name.into(),
        change,
        drift,
    })
}

/// Reports drift when the on-disk configuration does not match the persisted
/// manifest content fingerprint. Presence of a manifest without a config file
/// is also drift.
pub(crate) fn detect_drift(
    layout: &PowermiseLayout,
    context: &CompilationContext,
    name: &str,
    mise_config_dir: &Path,
) -> Result<Option<String>, LifecycleError> {
    let manifest = match load_manifest(layout, name)? {
        Some(manifest) => manifest,
        None => return Ok(None),
    };
    let config_path = context.artifact.config_path(mise_config_dir);
    let current = match std::fs::read(&config_path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(format!(
                "generated configuration `{}` is missing but manifest expects it",
                config_path.display()
            )));
        }
        Err(source) => {
            return Err(LifecycleError::Io {
                path: config_path,
                source,
            });
        }
    };
    let actual = powermise_state::content_fingerprint(&current);
    if actual != manifest.content_sha256 {
        return Ok(Some(format!(
            "generated configuration `{}` drifted from the persisted manifest (expected {}{}, got {})",
            config_path.display(),
            &manifest.content_sha256[..8],
            if actual.len() > 8 { "…" } else { "" },
            &actual[..8],
        )));
    }
    Ok(None)
}
