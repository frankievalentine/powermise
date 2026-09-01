//! `doctor`: report health findings for the registry, selection, ledger,
//! generated configuration, and mise executable. Never writes.
use std::path::Path;

use powermise_domain::Platform;
use powermise_mise::MiseExecutable;
use powermise_state::{content_fingerprint, load_ledger};

use crate::{
    CatalogPaths, PowermiseLayout, SelectionStore, compile_context, load_manifest,
};

/// Severity of a doctor finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindingSeverity {
    Ok,
    Warning,
    Error,
}

/// A single doctor finding.
pub struct Finding {
    pub severity: FindingSeverity,
    pub message: String,
}

impl Finding {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            severity: FindingSeverity::Ok,
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: FindingSeverity::Warning,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            severity: FindingSeverity::Error,
            message: message.into(),
        }
    }
}

/// Runs health checks over the managed selection, ledger, generated artifacts,
/// and the mise executable. Returns findings sorted by severity then message.
pub fn doctor(
    layout: &PowermiseLayout,
    paths: &CatalogPaths,
    name: &str,
    target: Platform,
    machine_role: Option<&str>,
    mise_config_dir: &Path,
    mise: &Path,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let store = SelectionStore::new(layout);

    match store.read(name) {
        Ok(_) => findings.push(Finding::ok(format!("selection `{name}` is present"))),
        Err(error) => {
            findings.push(Finding::error(format!("selection `{name}`: {error}")));
            return findings;
        }
    }

    match compile_context(paths, &layout.selection_path(name), target, machine_role) {
        Ok(context) => {
            findings.push(Finding::ok("selection resolves and compiles for target"));
            match crate::inspect_config(&context, name, mise_config_dir) {
                Ok(change) => findings.push(Finding::ok(format!(
                    "generated configuration is {kind:?}",
                    kind = change.kind
                ))),
                Err(error) => findings.push(Finding::error(format!(
                    "could not inspect generated configuration: {error}"
                ))),
            }
            match load_manifest(layout, name) {
                Ok(Some(manifest)) => {
                    let config_path = context.artifact.config_path(mise_config_dir);
                    match std::fs::read(&config_path) {
                        Ok(contents) if content_fingerprint(&contents) != manifest.content_sha256 => {
                            findings.push(Finding::warning(format!(
                                "generated configuration `{}` drifted from its manifest",
                                config_path.display()
                            )));
                        }
                        Ok(_) => findings.push(Finding::ok(
                            "generated configuration matches its manifest",
                        )),
                        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                            findings.push(Finding::warning(format!(
                                "generated configuration `{}` is missing despite a manifest",
                                config_path.display()
                            )));
                        }
                        Err(source) => findings.push(Finding::error(format!(
                            "could not read generated configuration: {source}"
                        ))),
                    }
                }
                Ok(None) => findings.push(Finding::warning(format!(
                    "no manifest recorded for `{name}`; run `powermise selection activate`"
                ))),
                Err(error) => findings.push(Finding::error(format!(
                    "could not load manifest: {error}"
                ))),
            }
        }
        Err(error) => findings.push(Finding::error(format!("selection does not compile: {error}"))),
    }

    match load_ledger(&layout.ledger_path) {
        Ok(ledger) => {
            if ledger.resources.is_empty() {
                findings.push(Finding::warning("ownership ledger is empty"));
            } else {
                findings.push(Finding::ok(format!(
                    "ownership ledger has {} entries",
                    ledger.resources.len()
                )));
            }
        }
        Err(error) => findings.push(Finding::error(format!("ownership ledger: {error}"))),
    }

    match MiseExecutable::new(mise).version() {
        Ok(version) => findings.push(Finding::ok(format!("mise {version} is available"))),
        Err(error) => findings.push(Finding::warning(format!("mise: {error}"))),
    }

    let _ = paths;
    findings.sort_by(|a, b| {
        severity_rank(a.severity)
            .cmp(&severity_rank(b.severity))
            .then_with(|| a.message.cmp(&b.message))
    });
    findings
}

fn severity_rank(severity: FindingSeverity) -> u8 {
    match severity {
        FindingSeverity::Ok => 0,
        FindingSeverity::Warning => 1,
        FindingSeverity::Error => 2,
    }
}

/// Reports whether a doctor run found any error-severity findings.
pub fn has_errors(findings: &[Finding]) -> bool {
    findings
        .iter()
        .any(|finding| finding.severity == FindingSeverity::Error)
}