//! Lazy-install stubs: small per-tool launcher scripts placed on `PATH` that
//! run a tool through the selection's mise environment, installing it on
//! first use (`mise x --env <env> <package> -- <command>`). This ports
//! Omarchy's lazy `~/.local/bin` stub pattern to Powermise's ownership model:
//! stubs are ledger-owned `Generated` artifacts, scoped to the selection
//! environment instead of the user's global mise config, and Powermise never
//! overwrites a stub file it does not own.
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use powermise_compiler::{StubEntry, compile_stubs};
use powermise_domain::{
    Operation, OperationAction, OperationOwnership, OperationProvider, OperationReversibility,
    OperationVerification, Platform, ResourceKind, ReversibilityKind, VerificationKind,
};
use powermise_registry::load_tool_catalog;
use powermise_state::{
    AtomicWriteResult, Ledger, content_fingerprint, load_ledger, write_atomically, write_ledger,
};

use crate::{
    CatalogPaths, LifecycleError, PowermiseLayout, compile_context, load_managed_selection,
    validate_selection_name,
};

/// The result of `install_stubs`.
#[derive(Debug)]
pub struct StubInstallReport {
    pub selection: String,
    pub bin_dir: PathBuf,
    /// Stub files written by this run.
    pub created: Vec<PathBuf>,
    /// Stub files that already matched the desired content.
    pub unchanged: Vec<PathBuf>,
    pub ledger_updated: bool,
}

/// The result of `remove_stubs`.
#[derive(Debug)]
pub struct StubRemoveReport {
    pub selection: String,
    pub dry_run: bool,
    /// Stub file paths that were (or, in dry-run, would be) removed.
    pub removed: Vec<PathBuf>,
    pub ledger_updated: bool,
}

/// Writes lazy-install stubs for every `mise-tool`-provided tool of a managed
/// selection. Requires the selection's generated configuration to exist
/// (activate or apply first), because the stubs execute against it. Idempotent:
/// unchanged stubs are left alone and the ledger converges.
pub fn install_stubs(
    layout: &PowermiseLayout,
    paths: &CatalogPaths,
    selection: &str,
    target: Platform,
    machine_role: Option<&str>,
    mise_config_dir: &Path,
    bin_dir_override: Option<&Path>,
) -> Result<StubInstallReport, LifecycleError> {
    validate_selection_name(selection)?;
    load_managed_selection(layout, selection)?;
    let context = compile_context(
        paths,
        &layout.selection_path(selection),
        target,
        machine_role,
    )?;
    let config_path = context.artifact.config_path(mise_config_dir);
    if !config_path.exists() {
        return Err(LifecycleError::NotActivated {
            name: selection.into(),
        });
    }
    let tool_catalog = load_tool_catalog(&paths.tools)?;
    let entries = compile_stubs(&context.resolved, &tool_catalog, target)?;

    let bin_dir = match bin_dir_override {
        Some(dir) => dir.to_path_buf(),
        None => resolve_home()?.join(".local/bin"),
    };
    fs::create_dir_all(&bin_dir).map_err(|source| LifecycleError::Io {
        path: bin_dir.clone(),
        source,
    })?;

    let mut ledger: Ledger = load_ledger(&layout.ledger_path)?;
    let mut created = Vec::new();
    let mut unchanged = Vec::new();
    for entry in &entries {
        let stub_path = stub_path_for(&bin_dir, entry, target);
        let contents = stub_contents(selection, entry, target);
        let fingerprint = content_fingerprint(contents.as_bytes());
        ensure_writable(&stub_path, &contents, selection, &ledger)?;

        let write_result = write_atomically(&stub_path, contents.as_bytes())?;
        if write_result != AtomicWriteResult::Unchanged {
            make_executable(&stub_path)?;
            created.push(stub_path.clone());
        } else {
            unchanged.push(stub_path.clone());
        }

        let observed_fingerprint = fs::read(&stub_path)
            .ok()
            .map(|bytes| content_fingerprint(&bytes));
        ledger.record_verified(
            &stub_operation(selection, entry, &stub_path, &fingerprint),
            observed_fingerprint,
        )?;
    }

    let ledger_updated =
        write_ledger(&layout.ledger_path, &ledger)? != AtomicWriteResult::Unchanged;
    Ok(StubInstallReport {
        selection: selection.into(),
        bin_dir,
        created,
        unchanged,
        ledger_updated,
    })
}

/// Removes the ledger-owned stubs of a selection. Dry-run by default
/// (`dry_run == true`); content outside Powermise ownership is never touched.
pub fn remove_stubs(
    layout: &PowermiseLayout,
    selection: &str,
    dry_run: bool,
) -> Result<StubRemoveReport, LifecycleError> {
    validate_selection_name(selection)?;
    let mut ledger: Ledger = load_ledger(&layout.ledger_path)?;
    let owned = ledger
        .resources
        .values()
        .filter(|record| {
            record.selection == selection
                && record.provider == OperationProvider::Generated
                && record.resource_kind == ResourceKind::StubScript
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut removed = Vec::new();
    for record in &owned {
        let path = PathBuf::from(&record.resource_id);
        removed.push(path.clone());
        if !dry_run {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(LifecycleError::Io { path, source });
                }
            }
            ledger.remove(&record.operation_id);
        }
    }

    let ledger_updated = if dry_run {
        false
    } else {
        write_ledger(&layout.ledger_path, &ledger)? != AtomicWriteResult::Unchanged
    };
    Ok(StubRemoveReport {
        selection: selection.into(),
        dry_run,
        removed,
        ledger_updated,
    })
}

/// Refuses to clobber a stub file Powermise does not own: an identical file is
/// converged silently, a differing file owned by another selection or by the
/// user is an error.
fn ensure_writable(
    stub_path: &Path,
    contents: &str,
    selection: &str,
    ledger: &Ledger,
) -> Result<(), LifecycleError> {
    let existing = match fs::read(stub_path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(LifecycleError::Io {
                path: stub_path.to_path_buf(),
                source,
            });
        }
    };
    if existing == contents.as_bytes() {
        return Ok(());
    }
    let resource_id = stub_path.to_string_lossy();
    if let Some(owner) = ledger
        .resources
        .values()
        .find(|record| record.resource_id == resource_id && record.selection != selection)
    {
        return Err(LifecycleError::Other {
            message: format!(
                "refusing to write stub `{}` for selection `{selection}`: \
                 it is already owned by selection `{}`",
                stub_path.display(),
                owner.selection
            ),
        });
    }
    let owned_here = ledger
        .resources
        .values()
        .any(|record| record.resource_id == resource_id && record.selection == selection);
    if !owned_here {
        return Err(LifecycleError::Other {
            message: format!(
                "refusing to overwrite `{}` because Powermise does not own it; \
                 pass a different --bin-dir",
                stub_path.display()
            ),
        });
    }
    Ok(())
}

fn stub_path_for(bin_dir: &Path, entry: &StubEntry, target: Platform) -> PathBuf {
    match target {
        Platform::Win32 => bin_dir.join(format!("{}.cmd", entry.command)),
        _ => bin_dir.join(&entry.command),
    }
}

/// The per-command recursion-guard variable. It is scoped to one command so a
/// tool may still reach other lazy stubs (for example `claude` running `rg`);
/// it only trips when PATH resolves a command back to its own stub because
/// mise did not provide it.
fn stub_guard_variable(command: &str) -> String {
    let suffix = command
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("POWERMISE_STUB_{suffix}")
}

fn stub_contents(selection: &str, entry: &StubEntry, target: Platform) -> String {
    let environment = format!("powermise-{selection}");
    let guard = stub_guard_variable(&entry.command);
    match target {
        Platform::Win32 => format!(
            "@echo off\r\n\
             rem Managed by Powermise for selection `{selection}`. Do not edit.\r\n\
             setlocal\r\n\
             if defined {guard} (\r\n\
             echo powermise: `{command}` was not installed by mise; run `powermise apply` 1>&2\r\n\
             exit /b 127\r\n\
             )\r\n\
             set {guard}=1\r\n\
             mise x --env {environment} {package} -- {command} %*\r\n",
            package = entry.package,
            command = entry.command,
        ),
        _ => format!(
            "#!/bin/sh\n\
             # Managed by Powermise for selection `{selection}`. Do not edit.\n\
             # If mise could not provide the tool, PATH resolves back to this stub;\n\
             # the guard stops that from recursing forever.\n\
             if [ -n \"${guard}\" ]; then\n\
             \x20 echo \"powermise: \\`{command}\\` was not installed by mise; run \\`powermise apply\\`\" >&2\n\
             \x20 exit 127\n\
             fi\n\
             {guard}=1 exec mise x --env {environment} {package} -- {command} \"$@\"\n",
            package = entry.package,
            command = entry.command,
        ),
    }
}

fn stub_operation(
    selection: &str,
    entry: &StubEntry,
    stub_path: &Path,
    fingerprint: &str,
) -> Operation {
    Operation {
        id: format!("stub:{selection}:{}", entry.command),
        provider: OperationProvider::Generated,
        resource_kind: ResourceKind::StubScript,
        resource_id: stub_path.to_string_lossy().into_owned(),
        action: OperationAction::Write,
        requested_state: fingerprint.to_owned(),
        depends_on: Vec::new(),
        ownership: OperationOwnership {
            owner: "powermise".into(),
            selection: selection.into(),
        },
        verification: OperationVerification {
            kind: VerificationKind::FileFingerprint,
            expected: fingerprint.to_owned(),
        },
        reversibility: OperationReversibility {
            kind: ReversibilityKind::DeclarationOnly,
            note: "remove the stub script".into(),
        },
    }
}

fn make_executable(stub_path: &Path) -> Result<(), LifecycleError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(stub_path)
            .map_err(|source| LifecycleError::Io {
                path: stub_path.to_path_buf(),
                source,
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(stub_path, permissions).map_err(|source| LifecycleError::Io {
            path: stub_path.to_path_buf(),
            source,
        })?;
    }
    #[cfg(not(unix))]
    {
        let _ = stub_path;
    }
    Ok(())
}

fn resolve_home() -> Result<PathBuf, LifecycleError> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or(LifecycleError::NoConfigDir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(package: &str, command: &str) -> StubEntry {
        StubEntry {
            tool_id: command.into(),
            package: package.into(),
            command: command.into(),
        }
    }

    #[test]
    fn unix_stubs_execute_through_the_selection_environment() {
        let contents = stub_contents(
            "engineering-agents",
            &entry("claude-code", "claude"),
            Platform::Darwin,
        );
        assert!(contents.starts_with("#!/bin/sh\n"));
        assert!(contents.contains("Managed by Powermise for selection `engineering-agents`"));
        assert!(contents.contains(
            "POWERMISE_STUB_CLAUDE=1 exec mise x --env powermise-engineering-agents claude-code -- claude \"$@\""
        ));
        assert!(contents.contains("if [ -n \"$POWERMISE_STUB_CLAUDE\" ]; then"));
    }

    #[test]
    fn stub_guards_are_scoped_to_one_command() {
        assert_eq!(stub_guard_variable("rg"), "POWERMISE_STUB_RG");
        assert_eq!(
            stub_guard_variable("gh-copilot"),
            "POWERMISE_STUB_GH_COPILOT"
        );
    }

    #[test]
    fn windows_stubs_are_cmd_scripts_with_crlf() {
        let contents = stub_contents(
            "engineering-agents",
            &entry("npm:@openai/codex", "codex"),
            Platform::Win32,
        );
        assert!(contents.starts_with("@echo off\r\n"));
        assert!(contents.ends_with("%*\r\n"));
        assert!(
            contents.contains(
                "mise x --env powermise-engineering-agents npm:@openai/codex -- codex %*"
            )
        );
    }

    #[test]
    fn windows_stub_files_use_the_cmd_extension() {
        let path = stub_path_for(Path::new("/bin"), &entry("jq", "jq"), Platform::Win32);
        assert_eq!(path, PathBuf::from("/bin/jq.cmd"));
        let path = stub_path_for(Path::new("/bin"), &entry("jq", "jq"), Platform::Linux);
        assert_eq!(path, PathBuf::from("/bin/jq"));
    }
}
