//! Powermise-owned native installers: winget, Scoop, npm, and verified scripts.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use powermise_domain::{
    NativeApplyOutput, NativeCheckOutput, NativeInstallRequest, NativeInstaller, OperationProvider,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NativeProviderError {
    #[error("unsupported native provider kind {0:?}")]
    UnsupportedKind(OperationProvider),
    #[error("could not execute `{executable}`: {source}")]
    Execute {
        executable: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`{executable}` failed with status {status:?}\n{message}")]
    Failed {
        executable: String,
        status: Option<i32>,
        message: String,
    },
    #[error("verified script requires a pinned immutable source URL and SHA-256")]
    UnverifiedScript,
    #[error(
        "verified script artifact `{artifact}` does not match pinned SHA-256 `{expected}`; got `{actual}`"
    )]
    IntegrityMismatch {
        artifact: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("could not verify or stage an artifact: {0}")]
    Artifact(String),
    #[error("could not read `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// The executable name for a package manager. On Windows, npm and Scoop are
/// `.cmd` shims, which `Command::new` does not resolve from a bare name.
fn shim_executable(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.into()
    }
}

fn run(executable: &str, arguments: &[String]) -> Result<Output, NativeProviderError> {
    let output = Command::new(executable)
        .args(arguments)
        .output()
        .map_err(|source| NativeProviderError::Execute {
            executable: executable.into(),
            source,
        })?;
    Ok(output)
}

/// The converged result for a package that is already present. Installing it
/// again is skipped: winget, for one, exits non-zero for an installed package,
/// which would make a repeat apply fail.
fn already_installed(package: &str, manager: &str) -> NativeApplyOutput {
    NativeApplyOutput {
        changed: false,
        message: format!("`{package}` is already installed via {manager}"),
    }
}

fn describe_output(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut message = String::new();
    if !stdout.is_empty() {
        message.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(&stderr);
    }
    message
}

fn required(provider: OperationProvider) -> Result<(), NativeProviderError> {
    match provider {
        OperationProvider::Winget
        | OperationProvider::Scoop
        | OperationProvider::Npm
        | OperationProvider::VerifiedScript => Ok(()),
        _ => Err(NativeProviderError::UnsupportedKind(provider)),
    }
}

/// The exact argument array for a noninteractive winget presence check.
pub fn winget_check_args(package: &str) -> Vec<String> {
    vec![
        "list".into(),
        "--id".into(),
        package.into(),
        "--exact".into(),
    ]
}

/// Whether a catalog version pins a specific release rather than tracking the
/// channel.
fn is_pinned(version: &str) -> bool {
    !version.is_empty() && version != "latest"
}

/// The exact argument array for a noninteractive winget install of the pinned
/// `version` (when one is pinned).
pub fn winget_install_args(package: &str, version: &str) -> Vec<String> {
    let mut arguments = vec![
        "install".into(),
        "--id".into(),
        package.into(),
        "--exact".into(),
    ];
    if is_pinned(version) {
        arguments.extend(["--version".into(), version.into()]);
    }
    arguments.extend([
        "--silent".into(),
        "--accept-package-agreements".into(),
        "--accept-source-agreements".into(),
    ]);
    arguments
}

/// The exact argument array for a Scoop presence check. `scoop prefix` exits
/// non-zero when the app is not installed, unlike `scoop list`, which succeeds
/// whether or not anything matched.
pub fn scoop_check_args(package: &str) -> Vec<String> {
    vec!["prefix".into(), package.into()]
}

/// The exact argument array for a Scoop install of the pinned `version`.
pub fn scoop_install_args(package: &str, version: &str) -> Vec<String> {
    let target = if is_pinned(version) {
        format!("{package}@{version}")
    } else {
        package.into()
    };
    vec!["install".into(), target]
}

/// The exact argument array for a global npm presence check.
pub fn npm_check_args(package: &str) -> Vec<String> {
    vec![
        "ls".into(),
        "--global".into(),
        "--depth=0".into(),
        package.into(),
    ]
}

/// The exact argument array for a global npm install.
pub fn npm_install_args(package: &str, version: &str) -> Vec<String> {
    if !is_pinned(version) {
        vec!["install".into(), "--global".into(), package.into()]
    } else {
        vec![
            "install".into(),
            "--global".into(),
            format!("{package}@{version}"),
        ]
    }
}

/// Renders the exact non-mutating command line for a native operation, used for dry-runs.
pub fn native_plan_command(
    provider: OperationProvider,
    package: &str,
    version: &str,
) -> Result<String, NativeProviderError> {
    required(provider)?;
    let (executable, arguments) = match provider {
        OperationProvider::Winget => ("winget", winget_install_args(package, version)),
        OperationProvider::Scoop => ("scoop", scoop_install_args(package, version)),
        OperationProvider::Npm => ("npm", npm_install_args(package, version)),
        OperationProvider::VerifiedScript => {
            return Ok(format!(
                "verify pinned immutable source and SHA-256, then execute staged script for `{package}`"
            ));
        }
        _ => unreachable!("required() rejects unsupported kinds"),
    };
    Ok(join_command(executable, &arguments))
}

/// Renders the exact presence-check command line for a native operation.
pub fn native_check_command(
    provider: OperationProvider,
    package: &str,
) -> Result<String, NativeProviderError> {
    required(provider)?;
    let (executable, arguments) = match provider {
        OperationProvider::Winget => ("winget", winget_check_args(package)),
        OperationProvider::Scoop => ("scoop", scoop_check_args(package)),
        OperationProvider::Npm => ("npm", npm_check_args(package)),
        OperationProvider::VerifiedScript => {
            return Ok(format!(
                "verify pinned immutable source and SHA-256 for `{package}`"
            ));
        }
        _ => unreachable!("required() rejects unsupported kinds"),
    };
    Ok(join_command(executable, &arguments))
}

fn join_command(executable: &str, arguments: &[String]) -> String {
    let mut command = executable.to_owned();
    for argument in arguments {
        command.push(' ');
        command.push_str(argument);
    }
    command
}

/// Installs packages through winget using exact IDs and noninteractive arguments.
#[derive(Clone, Debug)]
pub struct WingetProvider {
    executable: String,
}

impl WingetProvider {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
}

impl Default for WingetProvider {
    fn default() -> Self {
        Self::new("winget")
    }
}

impl NativeInstaller for WingetProvider {
    type Error = NativeProviderError;

    fn check(&self, request: &NativeInstallRequest) -> Result<NativeCheckOutput, Self::Error> {
        required(request.provider)?;
        let output = run(&self.executable, &winget_check_args(&request.package))?;
        if output.status.success() {
            Ok(NativeCheckOutput {
                present: true,
                detail: format!(
                    "`{}` reports `{}` installed",
                    self.executable, request.package
                ),
            })
        } else {
            Ok(NativeCheckOutput {
                present: false,
                detail: format!(
                    "`{}` reports `{}` not installed",
                    self.executable, request.package
                ),
            })
        }
    }

    fn plan(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
        required(request.provider)?;
        Ok(NativeApplyOutput {
            changed: true,
            message: native_plan_command(request.provider, &request.package, &request.version)?,
        })
    }

    fn apply(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
        required(request.provider)?;
        if self.check(request)?.present {
            return Ok(already_installed(&request.package, "winget"));
        }
        let output = run(
            &self.executable,
            &winget_install_args(&request.package, &request.version),
        )?;
        if !output.status.success() {
            return Err(NativeProviderError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                message: describe_output(&output),
            });
        }
        let present = self.check(request)?.present;
        Ok(NativeApplyOutput {
            changed: true,
            message: format!(
                "installed `{}` via winget; verified: {present}",
                request.package
            ),
        })
    }
}

/// Installs packages through Scoop after verifying the bucket is present.
#[derive(Clone, Debug)]
pub struct ScoopProvider {
    executable: String,
}

impl ScoopProvider {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
}

impl Default for ScoopProvider {
    fn default() -> Self {
        Self::new(shim_executable("scoop"))
    }
}

impl NativeInstaller for ScoopProvider {
    type Error = NativeProviderError;

    fn check(&self, request: &NativeInstallRequest) -> Result<NativeCheckOutput, Self::Error> {
        required(request.provider)?;
        let output = run(&self.executable, &scoop_check_args(&request.package))?;
        Ok(NativeCheckOutput {
            present: output.status.success(),
            detail: format!(
                "`{}` reports `{}` {}",
                self.executable,
                request.package,
                if output.status.success() {
                    "installed"
                } else {
                    "not installed"
                }
            ),
        })
    }

    fn plan(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
        required(request.provider)?;
        Ok(NativeApplyOutput {
            changed: true,
            message: native_plan_command(request.provider, &request.package, &request.version)?,
        })
    }

    fn apply(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
        required(request.provider)?;
        if self.check(request)?.present {
            return Ok(already_installed(&request.package, "Scoop"));
        }
        let output = run(
            &self.executable,
            &scoop_install_args(&request.package, &request.version),
        )?;
        if !output.status.success() {
            return Err(NativeProviderError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                message: describe_output(&output),
            });
        }
        let present = self.check(request)?.present;
        Ok(NativeApplyOutput {
            changed: true,
            message: format!(
                "installed `{}` via Scoop; verified: {present}",
                request.package
            ),
        })
    }
}

/// Installs global npm CLIs only where a mise tool resolution does not apply.
#[derive(Clone, Debug)]
pub struct NpmProvider {
    executable: String,
}

impl NpmProvider {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
}

impl Default for NpmProvider {
    fn default() -> Self {
        Self::new(shim_executable("npm"))
    }
}

impl NativeInstaller for NpmProvider {
    type Error = NativeProviderError;

    fn check(&self, request: &NativeInstallRequest) -> Result<NativeCheckOutput, Self::Error> {
        required(request.provider)?;
        let output = run(&self.executable, &npm_check_args(&request.package))?;
        Ok(NativeCheckOutput {
            present: output.status.success(),
            detail: format!(
                "`npm` reports `{}` {}",
                request.package,
                if output.status.success() {
                    "installed globally"
                } else {
                    "not installed globally"
                }
            ),
        })
    }

    fn plan(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
        required(request.provider)?;
        Ok(NativeApplyOutput {
            changed: true,
            message: native_plan_command(request.provider, &request.package, &request.version)?,
        })
    }

    fn apply(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
        required(request.provider)?;
        if self.check(request)?.present {
            return Ok(already_installed(&request.package, "npm"));
        }
        let output = run(
            &self.executable,
            &npm_install_args(&request.package, &request.version),
        )?;
        if !output.status.success() {
            return Err(NativeProviderError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                message: describe_output(&output),
            });
        }
        let present = self.check(request)?.present;
        Ok(NativeApplyOutput {
            changed: true,
            message: format!(
                "installed `{}` globally via npm; verified: {present}",
                request.package
            ),
        })
    }
}

/// Runs an immutable, integrity-verified script installer with a pinned source and SHA-256.
#[derive(Clone, Debug)]
pub struct VerifiedScriptProvider {
    /// Pinned immutable source URL.
    pub url: String,
    /// Pinned SHA-256 of the immutable artifact.
    pub sha256: String,
    /// The interpreter used to execute the staged artifact, e.g. `pwsh` or `sh`.
    pub shell: String,
    /// Directory where verified artifacts are staged before execution.
    pub artifact_dir: PathBuf,
}

impl VerifiedScriptProvider {
    pub fn new(
        url: impl Into<String>,
        sha256: impl Into<String>,
        shell: impl Into<String>,
    ) -> Self {
        let sha256 = sha256.into();
        Self {
            url: url.into(),
            sha256,
            shell: shell.into(),
            artifact_dir: std::env::temp_dir().join("powermise-verified-scripts"),
        }
    }

    fn artifact_path(&self) -> PathBuf {
        self.artifact_dir
            .join(format!("{}-{}", self.sha256, self.staged_name()))
    }

    fn staged_name(&self) -> String {
        if self.shell.contains("pwsh") || self.shell.contains("powershell") {
            "install.ps1".into()
        } else {
            "install.sh".into()
        }
    }

    fn staged_or_fetched(&self) -> Result<PathBuf, NativeProviderError> {
        let artifact = self.artifact_path();
        fs::create_dir_all(&self.artifact_dir).map_err(|source| {
            NativeProviderError::Artifact(format!(
                "could not create artifact directory `{}`: {source}",
                self.artifact_dir.display()
            ))
        })?;
        if !artifact.exists() || !verify_sha256(&artifact, &self.sha256)? {
            let output = run(
                "curl",
                &[
                    "--fail".into(),
                    "--location".into(),
                    "--silent".into(),
                    "--show-error".into(),
                    "--output".into(),
                    artifact.display().to_string(),
                    self.url.clone(),
                ],
            )?;
            if !output.status.success() {
                return Err(NativeProviderError::Artifact(format!(
                    "download of `{}` failed with status {:?}",
                    self.url,
                    output.status.code()
                )));
            }
        }
        if !verify_sha256(&artifact, &self.sha256)? {
            let contents = fs::read(&artifact).map_err(|source| NativeProviderError::Io {
                path: artifact.clone(),
                source,
            })?;
            return Err(NativeProviderError::IntegrityMismatch {
                artifact,
                expected: self.sha256.clone(),
                actual: powermise_state::content_fingerprint(&contents),
            });
        }
        Ok(artifact)
    }
}

impl NativeInstaller for VerifiedScriptProvider {
    type Error = NativeProviderError;

    fn check(&self, request: &NativeInstallRequest) -> Result<NativeCheckOutput, Self::Error> {
        required(request.provider)?;
        let artifact = self.artifact_path();
        let present = artifact.exists() && verify_sha256(&artifact, &self.sha256)?;
        Ok(NativeCheckOutput {
            present,
            detail: format!(
                "verified script artifact for `{}` is {}",
                request.package,
                if present {
                    "staged and integrity-verified"
                } else {
                    "not staged"
                }
            ),
        })
    }

    fn plan(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
        required(request.provider)?;
        Ok(NativeApplyOutput {
            changed: true,
            message: format!(
                "download `{}`, verify SHA-256 `{}`, then execute staged script with `{}` for `{}`",
                self.url, self.sha256, self.shell, request.package
            ),
        })
    }

    fn apply(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
        required(request.provider)?;
        let artifact = self.staged_or_fetched()?;
        let output = run(&self.shell, &[artifact.display().to_string()])?;
        if !output.status.success() {
            return Err(NativeProviderError::Failed {
                executable: self.shell.clone(),
                status: output.status.code(),
                message: describe_output(&output),
            });
        }
        Ok(NativeApplyOutput {
            changed: true,
            message: format!(
                "executed integrity-verified script for `{}` via `{}`",
                request.package, self.shell
            ),
        })
    }
}

/// Verifies that `path` matches the lowercase SHA-256 `expected`.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<bool, NativeProviderError> {
    let contents = fs::read(path).map_err(|source| NativeProviderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(powermise_state::content_fingerprint(&contents) == expected.to_lowercase())
}

#[cfg(test)]
pub mod fake {
    use std::collections::BTreeSet;

    use powermise_domain::{
        NativeApplyOutput, NativeCheckOutput, NativeInstallRequest, NativeInstaller,
    };

    use super::NativeProviderError;

    /// A deterministic installer used in tests where real package managers must not run.
    #[derive(Default)]
    pub struct FakeInstaller {
        pub present: BTreeSet<String>,
    }

    impl FakeInstaller {
        pub fn mark_present(&mut self, package: &str) {
            self.present.insert(package.into());
        }
    }

    impl NativeInstaller for FakeInstaller {
        type Error = NativeProviderError;

        fn check(&self, request: &NativeInstallRequest) -> Result<NativeCheckOutput, Self::Error> {
            let present = self.present.contains(&request.package);
            Ok(NativeCheckOutput {
                present,
                detail: format!("fake check for `{}`: {}", request.package, present),
            })
        }

        fn plan(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
            Ok(NativeApplyOutput {
                changed: !self.present.contains(&request.package),
                message: format!("fake plan for `{}`", request.package),
            })
        }

        fn apply(&self, request: &NativeInstallRequest) -> Result<NativeApplyOutput, Self::Error> {
            let changed = !self.present.contains(&request.package);
            Ok(NativeApplyOutput {
                changed,
                message: format!("fake apply for `{}`", request.package),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use powermise_domain::OperationProvider;

    fn request(provider: OperationProvider, package: &str) -> NativeInstallRequest {
        NativeInstallRequest {
            provider,
            package: package.into(),
            version: "latest".into(),
        }
    }

    #[test]
    fn winget_command_arguments_are_exact_and_noninteractive() {
        assert_eq!(
            winget_install_args("Git.Git", "latest"),
            [
                "install",
                "--id",
                "Git.Git",
                "--exact",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ]
        );
        assert_eq!(
            winget_check_args("Git.Git"),
            ["list", "--id", "Git.Git", "--exact"]
        );
    }

    #[test]
    fn pinned_versions_reach_winget_and_scoop() {
        assert_eq!(
            winget_install_args("Git.Git", "2.51.0"),
            [
                "install",
                "--id",
                "Git.Git",
                "--exact",
                "--version",
                "2.51.0",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ]
        );
        assert_eq!(
            scoop_install_args("zed", "0.200.0"),
            ["install", "zed@0.200.0"]
        );
        assert_eq!(scoop_install_args("zed", "latest"), ["install", "zed"]);
        assert_eq!(scoop_check_args("zed"), ["prefix", "zed"]);
    }

    #[test]
    fn plan_commands_are_non_mutating_and_rendered() {
        let winget = WingetProvider::default();
        let plan = winget
            .plan(&request(OperationProvider::Winget, "Git.Git"))
            .unwrap();
        assert!(plan.changed);
        assert_eq!(
            plan.message,
            "winget install --id Git.Git --exact --silent --accept-package-agreements --accept-source-agreements"
        );

        let npm = NpmProvider::default();
        let plan = npm
            .plan(&request(OperationProvider::Npm, "@openai/codex"))
            .unwrap();
        assert!(
            plan.message
                .starts_with("npm install --global @openai/codex")
        );
    }

    #[test]
    fn unsupported_provider_kinds_are_rejected() {
        let request = NativeInstallRequest {
            provider: OperationProvider::MiseTools,
            package: "node".into(),
            version: "latest".into(),
        };
        let winget = WingetProvider::default();

        assert!(matches!(
            winget.check(&request),
            Err(NativeProviderError::UnsupportedKind(
                OperationProvider::MiseTools
            ))
        ));
    }

    #[test]
    fn fake_installer_reports_presence_and_plan() {
        let mut installer = fake::FakeInstaller::default();
        installer.mark_present("git");

        assert!(
            installer
                .check(&request(OperationProvider::Winget, "git"))
                .unwrap()
                .present
        );
        assert!(
            !installer
                .check(&request(OperationProvider::Winget, "curl"))
                .unwrap()
                .present
        );
    }

    #[test]
    fn sha256_verification_matches_pinned_digest() {
        let directory =
            std::env::temp_dir().join(format!("powermise-verify-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let artifact = directory.join("script.sh");
        fs::write(&artifact, b"#!/bin/sh\nexit 0\n").unwrap();

        let contents = fs::read(&artifact).unwrap();
        let digest = powermise_state::content_fingerprint(&contents);

        assert!(verify_sha256(&artifact, &digest).unwrap());
        assert!(!verify_sha256(&artifact, &"0".repeat(64)).unwrap());
        let _ = fs::remove_dir_all(&directory);
    }
}
