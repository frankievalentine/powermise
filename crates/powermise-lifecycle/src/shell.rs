//! Opt-in shell integration: installs or removes a marker-guarded block in a
//! shell startup file that exports the selection's mise environment. The block
//! is fully managed by Powermise and uninstall never touches content outside
//! the markers.
use std::{
    env, fs,
    path::{Path, PathBuf},
};

use powermise_state::write_atomically;

use crate::LifecycleError;

/// The shells Powermise can integrate with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Zsh,
    Bash,
    Powershell,
}

impl Shell {
    /// Detects the user's shell from `$SHELL`, defaulting to `zsh` on
    /// Unix-like systems and PowerShell on Windows when the variable is
    /// absent or unrecognized.
    pub fn detect() -> Shell {
        let shell = env::var_os("SHELL")
            .map(PathBuf::from)
            .and_then(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        if shell.contains("powershell") || shell.contains("pwsh") {
            return Shell::Powershell;
        }
        if cfg!(windows) {
            return Shell::Powershell;
        }
        if shell.contains("bash") {
            Shell::Bash
        } else {
            Shell::Zsh
        }
    }

    /// Resolves the startup file to manage. A provided `home` (used in tests)
    /// fully determines the path; when absent, `$HOME` (and `$ZDOTDIR` where
    /// defined) drive the resolution. PowerShell integrates with the pwsh
    /// current-user all-hosts profile (see `powershell_profile`).
    pub fn rc_path(&self, home: Option<&Path>) -> Result<PathBuf, LifecycleError> {
        match home {
            Some(path) => Ok(match self {
                Shell::Zsh => path.join(".zshrc"),
                Shell::Bash => path.join(".bashrc"),
                Shell::Powershell => powershell_profile(path),
            }),
            None => {
                let home = env::var_os("HOME")
                    .or_else(|| env::var_os("USERPROFILE"))
                    .map(PathBuf::from)
                    .ok_or(LifecycleError::NoConfigDir)?;
                Ok(match self {
                    Shell::Zsh => env::var_os("ZDOTDIR")
                        .map(PathBuf::from)
                        .unwrap_or(home)
                        .join(".zshrc"),
                    Shell::Bash => home.join(".bashrc"),
                    Shell::Powershell => powershell_profile(&home),
                })
            }
        }
    }

    /// The activation line that selects the selection's mise environment.
    fn activation_line(&self, selection: &str) -> String {
        match self {
            Shell::Zsh | Shell::Bash => format!("export MISE_ENV=powermise-{selection}"),
            Shell::Powershell => format!("$env:MISE_ENV = \"powermise-{selection}\""),
        }
    }
}

/// The result of `install_shell_integration`.
#[derive(Debug)]
pub struct ShellInstallReport {
    pub rc_path: PathBuf,
    /// Whether the managed block was newly written (`false` when already
    /// present, making install idempotent).
    pub inserted: bool,
}

/// The result of `uninstall_shell_integration`.
#[derive(Debug)]
pub struct ShellUninstallReport {
    pub rc_path: PathBuf,
    pub dry_run: bool,
    /// Whether a managed block was found (and removed unless in dry-run).
    pub removed: bool,
}

/// The pwsh current-user, all-hosts profile: `Documents\\PowerShell` on
/// Windows and `~/.config/powershell` elsewhere.
fn powershell_profile(home: &Path) -> PathBuf {
    if cfg!(windows) {
        home.join("Documents")
            .join("PowerShell")
            .join("profile.ps1")
    } else {
        home.join(".config/powershell/profile.ps1")
    }
}

/// Validates a selection name for use in the environment name and markers.
fn validate_name(selection: &str) -> Result<(), LifecycleError> {
    crate::validate_selection_name(selection)
}

fn start_marker(selection: &str) -> String {
    format!("# >>> powermise:{selection} >>>")
}

fn end_marker(selection: &str) -> String {
    format!("# <<< powermise:{selection} <<<")
}

/// The selection whose managed block is present in `contents`, if any.
fn installed_selection(contents: &str) -> Option<&str> {
    contents.lines().find_map(|line| {
        line.strip_prefix("# >>> powermise:")
            .and_then(|rest| rest.strip_suffix(" >>>"))
    })
}

/// Builds the full managed block including markers.
fn managed_block(shell: &Shell, selection: &str) -> String {
    format!(
        "{}\n# Managed by Powermise. Do not edit.\n{}\n{}",
        start_marker(selection),
        shell.activation_line(selection),
        end_marker(selection),
    )
}

fn read_rc(path: &Path) -> Result<String, LifecycleError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(source) => Err(LifecycleError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn locate_block(contents: &str, selection: &str) -> Option<(usize, usize)> {
    let start = start_marker(selection);
    let end = end_marker(selection);
    let lines = contents.lines().collect::<Vec<_>>();
    let start_index = lines.iter().position(|line| *line == start)?;
    let end_index = lines
        .iter()
        .skip(start_index + 1)
        .position(|line| *line == end)
        .map(|offset| start_index + 1 + offset)?;
    Some((start_index, end_index))
}

/// Appends the managed block to a selection's shell startup file. Idempotent:
/// a second install is a no-op.
pub fn install_shell_integration(
    shell: &Shell,
    selection: &str,
    home: Option<&Path>,
) -> Result<ShellInstallReport, LifecycleError> {
    validate_name(selection)?;
    let rc_path = shell.rc_path(home)?;
    let existing = read_rc(&rc_path)?;
    if existing.contains(&start_marker(selection)) {
        return Ok(ShellInstallReport {
            rc_path,
            inserted: false,
        });
    }
    // `MISE_ENV` holds one environment, so a second block would silently
    // override the first. Only one selection may be active per shell.
    if let Some(other) = installed_selection(&existing) {
        return Err(LifecycleError::Other {
            message: format!(
                "`{}` already activates selection `{other}`; run \
                 `powermise shell uninstall --name {other} --yes` first",
                rc_path.display()
            ),
        });
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&managed_block(shell, selection));
    updated.push('\n');
    write_atomically(&rc_path, updated.as_bytes())?;
    Ok(ShellInstallReport {
        rc_path,
        inserted: true,
    })
}

/// Removes a selection's managed block from its shell startup file. Dry-run by
/// default (`apply == false`); `apply == true` performs the removal. Content
/// outside the markers is never modified.
pub fn uninstall_shell_integration(
    shell: &Shell,
    selection: &str,
    home: Option<&Path>,
    apply: bool,
) -> Result<ShellUninstallReport, LifecycleError> {
    validate_name(selection)?;
    let rc_path = shell.rc_path(home)?;
    let existing = read_rc(&rc_path)?;
    let Some((start_index, end_index)) = locate_block(&existing, selection) else {
        return Ok(ShellUninstallReport {
            rc_path,
            dry_run: !apply,
            removed: false,
        });
    };
    if !apply {
        return Ok(ShellUninstallReport {
            rc_path,
            dry_run: true,
            removed: true,
        });
    }
    let mut updated = String::new();
    for (index, line) in existing.lines().enumerate() {
        if index >= start_index && index <= end_index {
            continue;
        }
        updated.push_str(line);
        updated.push('\n');
    }
    write_atomically(&rc_path, updated.as_bytes())?;
    Ok(ShellUninstallReport {
        rc_path,
        dry_run: false,
        removed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home_dir() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("powermise-shell-test-{unique}"));
        fs::create_dir_all(&dir).expect("home directory should be created");
        dir
    }

    #[test]
    fn install_writes_a_marker_guarded_block() {
        let home = home_dir();
        let report = install_shell_integration(&Shell::Zsh, "engineering-agents", Some(&home))
            .expect("install should succeed");
        assert!(report.inserted);
        let rc = fs::read_to_string(&report.rc_path).expect("rc file should be readable");
        assert!(rc.contains(&start_marker("engineering-agents")));
        assert!(rc.contains(&end_marker("engineering-agents")));
        assert!(rc.contains("export MISE_ENV=powermise-engineering-agents"));
    }

    #[test]
    fn install_is_idempotent() {
        let home = home_dir();
        install_shell_integration(&Shell::Bash, "engineering-agents", Some(&home))
            .expect("first install should succeed");
        let report = install_shell_integration(&Shell::Bash, "engineering-agents", Some(&home))
            .expect("second install should succeed");
        assert!(!report.inserted, "a repeated install must be a no-op");
        let rc = fs::read_to_string(&report.rc_path).unwrap();
        let occurrences = rc.matches(&start_marker("engineering-agents")).count();
        assert_eq!(occurrences, 1, "the managed block must not be duplicated");
    }

    #[test]
    fn uninstall_dry_run_leaves_the_file_untouched() {
        let home = home_dir();
        install_shell_integration(&Shell::Zsh, "engineering-agents", Some(&home))
            .expect("install should succeed");
        let rc_path = Shell::Zsh.rc_path(Some(&home)).unwrap();
        let before = fs::read(&rc_path).unwrap();
        let report =
            uninstall_shell_integration(&Shell::Zsh, "engineering-agents", Some(&home), false)
                .expect("dry-run uninstall should succeed");
        assert!(report.dry_run);
        assert!(report.removed);
        assert_eq!(
            fs::read(&rc_path).unwrap(),
            before,
            "dry-run uninstall must not modify the file"
        );
    }

    #[test]
    fn uninstall_removes_only_the_managed_block() {
        let home = home_dir();
        let rc_path = Shell::Bash.rc_path(Some(&home)).unwrap();
        fs::write(&rc_path, "# my own alias\nalias g='git'\n").unwrap();
        install_shell_integration(&Shell::Bash, "engineering-agents", Some(&home))
            .expect("install should succeed");
        let report =
            uninstall_shell_integration(&Shell::Bash, "engineering-agents", Some(&home), true)
                .expect("uninstall should succeed");
        assert!(!report.dry_run);
        assert!(report.removed);
        let rc = fs::read_to_string(&rc_path).unwrap();
        assert!(!rc.contains(&start_marker("engineering-agents")));
        assert!(!rc.contains("MISE_ENV=powermise-engineering-agents"));
        assert!(
            rc.contains("# my own alias"),
            "user content must be preserved"
        );
        assert!(
            rc.contains("alias g='git'"),
            "user content must be preserved"
        );
    }

    #[test]
    fn uninstall_with_no_block_reports_nothing_removed() {
        let home = home_dir();
        let report =
            uninstall_shell_integration(&Shell::Zsh, "engineering-agents", Some(&home), true)
                .expect("uninstall should succeed");
        assert!(!report.removed);
    }

    #[test]
    fn install_refuses_a_second_active_selection() {
        let home = home_dir();
        install_shell_integration(&Shell::Zsh, "first", Some(&home)).expect("first install");
        let error = install_shell_integration(&Shell::Zsh, "second", Some(&home))
            .expect_err("a second selection must be refused");
        assert!(
            error
                .to_string()
                .contains("already activates selection `first`")
        );
    }

    #[test]
    fn invalid_selection_names_are_rejected() {
        let home = home_dir();
        let error = install_shell_integration(&Shell::Zsh, "bad name", Some(&home))
            .expect_err("a space in the name must be rejected");
        assert!(error.to_string().contains("invalid selection name"));
    }

    #[test]
    fn powershell_profile_matches_where_pwsh_reads_it() {
        let home = Path::new("/home/user");
        let expected = if cfg!(windows) {
            home.join("Documents/PowerShell/profile.ps1")
        } else {
            home.join(".config/powershell/profile.ps1")
        };
        assert_eq!(powershell_profile(home), expected);
    }

    #[test]
    fn install_writes_a_powershell_block() {
        let home = home_dir();
        let report =
            install_shell_integration(&Shell::Powershell, "engineering-agents", Some(&home))
                .expect("install should succeed");
        assert!(report.inserted);
        assert_eq!(
            report.rc_path,
            powershell_profile(&home),
            "PowerShell integrates with the pwsh all-hosts profile"
        );
        let profile = fs::read_to_string(&report.rc_path).expect("profile should be readable");
        assert!(profile.contains(&start_marker("engineering-agents")));
        assert!(profile.contains(&end_marker("engineering-agents")));
        assert!(profile.contains("$env:MISE_ENV = \"powermise-engineering-agents\""));
    }

    #[test]
    fn powershell_install_is_idempotent() {
        let home = home_dir();
        install_shell_integration(&Shell::Powershell, "engineering-agents", Some(&home))
            .expect("first install should succeed");
        let report =
            install_shell_integration(&Shell::Powershell, "engineering-agents", Some(&home))
                .expect("second install should succeed");
        assert!(!report.inserted, "a repeated install must be a no-op");
        let profile = fs::read_to_string(&report.rc_path).unwrap();
        let occurrences = profile.matches(&start_marker("engineering-agents")).count();
        assert_eq!(occurrences, 1, "the managed block must not be duplicated");
    }

    #[test]
    fn powershell_uninstall_removes_only_the_managed_block() {
        let home = home_dir();
        let profile_path = Shell::Powershell.rc_path(Some(&home)).unwrap();
        fs::create_dir_all(profile_path.parent().unwrap()).unwrap();
        fs::write(
            &profile_path,
            "# my own profile\nSet-PSReadLineOption -HistoryNoDuplicates\n",
        )
        .unwrap();
        install_shell_integration(&Shell::Powershell, "engineering-agents", Some(&home))
            .expect("install should succeed");
        let report = uninstall_shell_integration(
            &Shell::Powershell,
            "engineering-agents",
            Some(&home),
            true,
        )
        .expect("uninstall should succeed");
        assert!(report.removed);
        let profile = fs::read_to_string(&profile_path).unwrap();
        assert!(!profile.contains(&start_marker("engineering-agents")));
        assert!(!profile.contains("MISE_ENV"));
        assert!(
            profile.contains("Set-PSReadLineOption -HistoryNoDuplicates"),
            "user content must be preserved"
        );
    }
}
