use std::{
    ffi::OsStr,
    fs, io,
    path::{Component, Path, PathBuf},
    process::{self, Command},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

mod client;
mod models;

pub use client::{MiseClient, MiseCommand, OutputLine, RunOutcome, Stream};
pub use models::{
    ConfigFile, ConfigSource, Doctor, MiseVersion, OutdatedReport, OutdatedTool, RegistryEntry,
    RemoteVersion, ShellInfo, Task, ToolInventory, ToolVersion, VersionInfo,
};

use powermise_domain::{
    MiseBootstrapApplyOutput, MiseBootstrapStatus, MiseDryRunOutput, MiseDryRunRequest,
    MiseEnvironmentRequest, MiseRunner,
};
use thiserror::Error;

static SANDBOX_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct MiseExecutable {
    executable: PathBuf,
}

impl MiseExecutable {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    /// Returns the mise version string, for example `2026.7.18 macos-arm64`.
    pub fn version(&self) -> Result<String, MiseAdapterError> {
        let output = Command::new(&self.executable)
            .arg("--version")
            .output()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(MiseAdapterError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                stdout,
                stderr,
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// Runs `command` inside the selection environment with inherited stdio.
    pub fn exec(
        &self,
        environment: &str,
        command: &[String],
    ) -> Result<process::ExitStatus, MiseAdapterError> {
        validate_environment_name(environment)?;
        let mut process = Command::new(&self.executable);
        process
            .args(["--env", environment])
            .arg("exec")
            .arg("--")
            .args(command);
        process
            .status()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })
    }

    fn command(&self, request: &MiseDryRunRequest, sandbox: &Sandbox) -> Command {
        self.base_command(request, sandbox, &["bootstrap", "--dry-run"])
    }

    /// Builds the real convergence command. Unlike the sandboxed dry run this
    /// deliberately keeps the machine's own HOME and mise data directories:
    /// `bootstrap --yes` must install into the real environment.
    fn bootstrap_apply_command(&self, request: &MiseEnvironmentRequest) -> Command {
        self.live_command(request, &["bootstrap", "--yes"])
    }

    /// Builds the read-only status probe against the real environment, so it
    /// reports what is actually installed rather than an empty sandbox.
    fn live_status_command(&self, request: &MiseEnvironmentRequest) -> Command {
        self.live_command(request, &["bootstrap", "status", "--json"])
    }

    /// Runs against the machine's real HOME and data directories. Only the
    /// config directory is pinned so the command sees exactly the directory
    /// powermise wrote the generated configuration to, and the working
    /// directory is pinned to that same directory so an unrelated
    /// project-local `mise.toml` can never join the run.
    fn live_command(&self, request: &MiseEnvironmentRequest, arguments: &[&str]) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .args(["--env", &request.environment])
            .args(arguments)
            .current_dir(&request.config_dir)
            .env("MISE_CONFIG_DIR", &request.config_dir)
            .env_remove("MISE_CONFIG_FILE")
            .env_remove("MISE_GLOBAL_CONFIG_FILE")
            .env_remove("MISE_ENV");
        command
    }

    fn base_command(
        &self,
        request: &MiseDryRunRequest,
        sandbox: &Sandbox,
        arguments: &[&str],
    ) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .args(["--env", &request.environment])
            .args(arguments)
            .current_dir(&sandbox.home)
            .env("HOME", &sandbox.home)
            .env("MISE_CONFIG_DIR", &sandbox.config)
            .env("MISE_DATA_DIR", &sandbox.data)
            .env("MISE_CACHE_DIR", &sandbox.cache)
            .env("MISE_STATE_DIR", &sandbox.state)
            .env("MISE_SYSTEM_CONFIG_DIR", &sandbox.system)
            .env_remove("MISE_CONFIG_FILE")
            .env_remove("MISE_GLOBAL_CONFIG_FILE")
            .env_remove("MISE_ENV");
        command
    }
}

impl MiseRunner for MiseExecutable {
    type Error = MiseAdapterError;

    fn bootstrap_dry_run(
        &self,
        request: &MiseDryRunRequest,
    ) -> Result<MiseDryRunOutput, Self::Error> {
        validate_config_file_name(&request.config_file_name)?;
        let sandbox = Sandbox::create()?;
        let config_path = sandbox.config.join(&request.config_file_name);
        fs::write(&config_path, request.config_contents.as_bytes()).map_err(|source| {
            MiseAdapterError::Io {
                operation: "write isolated generated configuration",
                path: config_path,
                source,
            }
        })?;

        let output = self.command(request, &sandbox).output().map_err(|source| {
            MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            }
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !output.status.success() {
            return Err(MiseAdapterError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                stdout,
                stderr,
            });
        }

        Ok(MiseDryRunOutput { stdout, stderr })
    }

    fn bootstrap_status(
        &self,
        request: &MiseEnvironmentRequest,
    ) -> Result<MiseBootstrapStatus, Self::Error> {
        validate_environment_name(&request.environment)?;
        let output = self
            .live_status_command(request)
            .output()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(MiseAdapterError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                stdout,
                stderr,
            });
        }

        serde_json::from_slice(&output.stdout).map_err(|source| MiseAdapterError::Status {
            executable: self.executable.clone(),
            source,
        })
    }

    fn bootstrap_apply(
        &self,
        request: &MiseEnvironmentRequest,
    ) -> Result<MiseBootstrapApplyOutput, Self::Error> {
        validate_environment_name(&request.environment)?;
        let output = self
            .bootstrap_apply_command(request)
            .output()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !output.status.success() {
            return Err(MiseAdapterError::BootstrapFailed {
                executable: self.executable.clone(),
                status: output.status.code(),
                stdout,
                stderr,
            });
        }

        Ok(MiseBootstrapApplyOutput { stdout, stderr })
    }

    fn exec(
        &self,
        environment: &str,
        command: &[String],
    ) -> Result<process::ExitStatus, Self::Error> {
        MiseExecutable::exec(self, environment, command)
    }
}

#[derive(Debug, Error)]
pub enum MiseAdapterError {
    #[error("invalid generated mise config file name `{file_name}`")]
    InvalidConfigFileName { file_name: String },
    #[error("invalid mise environment name `{environment}`")]
    InvalidEnvironment { environment: String },
    #[error("could not {operation} `{path}`: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not execute mise at `{executable}`: {source}")]
    Execute {
        executable: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "mise at `{executable}` failed with status {status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    Failed {
        executable: PathBuf,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "real `mise bootstrap --yes` at `{executable}` failed with status {status:?}; applied operations were not rolled back\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    BootstrapFailed {
        executable: PathBuf,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("could not parse the JSON from `{command}`: {source}")]
    Json {
        command: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("`{value}` is not a valid tool name or version")]
    InvalidArgument { value: String },
    #[error("could not parse `mise bootstrap status --json` from `{executable}`: {source}")]
    Status {
        executable: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

struct Sandbox {
    root: PathBuf,
    home: PathBuf,
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
    state: PathBuf,
    system: PathBuf,
}

impl Sandbox {
    fn create() -> Result<Self, MiseAdapterError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = SANDBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "powermise-plan-{}-{timestamp}-{sequence}",
            process::id(),
        ));
        let sandbox = Self {
            home: root.join("home"),
            config: root.join("config"),
            data: root.join("data"),
            cache: root.join("cache"),
            state: root.join("state"),
            system: root.join("system"),
            root,
        };

        for directory in [
            &sandbox.home,
            &sandbox.config,
            &sandbox.data,
            &sandbox.cache,
            &sandbox.state,
            &sandbox.system,
        ] {
            fs::create_dir_all(directory).map_err(|source| MiseAdapterError::Io {
                operation: "create isolated mise directory",
                path: directory.clone(),
                source,
            })?;
        }

        Ok(sandbox)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn validate_config_file_name(file_name: &str) -> Result<(), MiseAdapterError> {
    let path = Path::new(file_name);
    let mut components = path.components();
    let valid = matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && path.extension() == Some(OsStr::new("toml"));

    if valid {
        Ok(())
    } else {
        Err(MiseAdapterError::InvalidConfigFileName {
            file_name: file_name.into(),
        })
    }
}

fn validate_environment_name(environment: &str) -> Result<(), MiseAdapterError> {
    let valid = !environment.is_empty()
        && environment
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');

    if valid {
        Ok(())
    } else {
        Err(MiseAdapterError::InvalidEnvironment {
            environment: environment.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(file_name: &str) -> MiseDryRunRequest {
        MiseDryRunRequest {
            environment: "powermise-test".into(),
            config_file_name: file_name.into(),
            config_contents: "[tools]\nnode = \"24\"\n".into(),
        }
    }

    #[test]
    fn rejects_config_paths_outside_the_sandbox_config_root() {
        let runner = MiseExecutable::new("mise");

        for file_name in ["../config.toml", "nested/config.toml", "config.txt", ""] {
            assert!(matches!(
                runner.bootstrap_dry_run(&request(file_name)),
                Err(MiseAdapterError::InvalidConfigFileName { .. })
            ));
        }
    }

    #[test]
    fn rejects_unsafe_environment_names_for_exec() {
        let runner = MiseExecutable::new("mise");

        for environment in ["../powermise-x", "powermise x", "POWE", "", "powermise;rm"] {
            assert!(matches!(
                runner.exec(environment, &["mise".into(), "ls".into()]),
                Err(MiseAdapterError::InvalidEnvironment { .. })
            ));
        }
    }

    #[test]
    fn command_uses_only_isolated_mise_directories() {
        let runner = MiseExecutable::new("mise");
        let sandbox = Sandbox::create().expect("sandbox should be created");
        let command = runner.command(&request("config.powermise-test.toml"), &sandbox);
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let environment = command
            .get_envs()
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(
            arguments,
            ["--env", "powermise-test", "bootstrap", "--dry-run"]
        );
        assert_eq!(
            environment.get(OsStr::new("HOME")),
            Some(&Some(sandbox.home.as_os_str()))
        );
        assert_eq!(
            environment.get(OsStr::new("MISE_CONFIG_DIR")),
            Some(&Some(sandbox.config.as_os_str()))
        );
        assert_eq!(environment.get(OsStr::new("MISE_ENV")), Some(&None));
    }

    #[test]
    fn bootstrap_apply_targets_the_real_config_directory_without_sandboxing_home() {
        let runner = MiseExecutable::new("mise");
        let config_dir = std::env::temp_dir().join("powermise-bootstrap-apply-test");
        let request = MiseEnvironmentRequest {
            environment: "powermise-test".into(),
            config_dir: config_dir.clone(),
        };
        let command = runner.bootstrap_apply_command(&request);
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let environment = command
            .get_envs()
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(arguments, ["--env", "powermise-test", "bootstrap", "--yes"]);
        assert_eq!(command.get_current_dir(), Some(config_dir.as_path()));
        assert_eq!(
            environment.get(OsStr::new("MISE_CONFIG_DIR")),
            Some(&Some(config_dir.as_os_str()))
        );
        assert!(!environment.contains_key(OsStr::new("HOME")));
        assert_eq!(environment.get(OsStr::new("MISE_ENV")), Some(&None));
    }

    #[test]
    fn status_probes_the_real_environment_read_only() {
        let runner = MiseExecutable::new("mise");
        let config_dir = std::env::temp_dir().join("powermise-status-test");
        let request = MiseEnvironmentRequest {
            environment: "powermise-test".into(),
            config_dir: config_dir.clone(),
        };
        let command = runner.live_status_command(&request);
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let environment = command
            .get_envs()
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(
            arguments,
            ["--env", "powermise-test", "bootstrap", "status", "--json"]
        );
        assert!(!environment.contains_key(OsStr::new("HOME")));
        assert!(!environment.contains_key(OsStr::new("MISE_DATA_DIR")));
        assert_eq!(
            environment.get(OsStr::new("MISE_CONFIG_DIR")),
            Some(&Some(config_dir.as_os_str()))
        );
    }

    #[test]
    fn bootstrap_apply_rejects_unsafe_environment_names() {
        let runner = MiseExecutable::new("mise");
        let request = MiseEnvironmentRequest {
            environment: "powermise x;rm".into(),
            config_dir: std::env::temp_dir(),
        };

        assert!(matches!(
            runner.bootstrap_apply(&request),
            Err(MiseAdapterError::InvalidEnvironment { .. })
        ));
    }
}
