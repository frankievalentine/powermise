//! A typed client for the installed `mise` executable. Reads run mise's JSON
//! commands; changes are expressed as [`MiseCommand`]s that render the exact
//! command line shown to people and run with streamed output. powermise never
//! edits mise configuration itself.
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use serde::de::DeserializeOwned;

use crate::{
    MiseAdapterError,
    models::{
        ConfigFile, Doctor, OutdatedReport, RegistryEntry, RemoteVersion, Task, ToolInventory,
        VersionInfo,
    },
};

/// The installed mise, run from a fixed working directory so project-local
/// configs only join when powermise asks for a project.
#[derive(Clone, Debug)]
pub struct MiseClient {
    executable: PathBuf,
    /// The directory reads run in when no project is given (the home directory,
    /// so the global view isn't shaped by wherever powermise was launched).
    home: PathBuf,
}

impl MiseClient {
    pub fn new(executable: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            home: home.into(),
        }
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    fn base(&self, cwd: Option<&Path>) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .current_dir(cwd.unwrap_or(&self.home))
            .stdin(Stdio::null())
            // Plain output: no colors or spinners in captured text.
            .env("NO_COLOR", "1")
            .env("MISE_COLOR", "0");
        command
    }

    fn read_json<T: DeserializeOwned>(
        &self,
        arguments: &[&str],
        cwd: Option<&Path>,
    ) -> Result<T, MiseAdapterError> {
        let output = self.base(cwd).args(arguments).output().map_err(|source| {
            MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            }
        })?;
        if !output.status.success() {
            return Err(MiseAdapterError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        serde_json::from_slice(&output.stdout).map_err(|source| MiseAdapterError::Json {
            command: format!("mise {}", arguments.join(" ")),
            source,
        })
    }

    /// `mise version --json`.
    pub fn version_info(&self) -> Result<VersionInfo, MiseAdapterError> {
        self.read_json(&["version", "--json"], None)
    }

    /// `mise ls --json`, from the home directory or a project.
    pub fn tools(&self, project: Option<&Path>) -> Result<ToolInventory, MiseAdapterError> {
        self.read_json(&["ls", "--json"], project)
    }

    /// `mise outdated --bump --json`: includes newer versions beyond pins.
    pub fn outdated(&self, project: Option<&Path>) -> Result<OutdatedReport, MiseAdapterError> {
        self.read_json(&["outdated", "--bump", "--json"], project)
    }

    /// `mise registry --json`: every tool mise knows how to install.
    pub fn registry(&self) -> Result<Vec<RegistryEntry>, MiseAdapterError> {
        self.read_json(&["registry", "--json"], None)
    }

    /// `mise ls-remote --json <tool>`: versions available to install.
    pub fn remote_versions(&self, tool: &str) -> Result<Vec<RemoteVersion>, MiseAdapterError> {
        validate_argument(tool)?;
        self.read_json(&["ls-remote", "--json", tool], None)
    }

    /// `mise config ls --json` for a project (or the global view).
    pub fn config_files(
        &self,
        project: Option<&Path>,
    ) -> Result<Vec<ConfigFile>, MiseAdapterError> {
        self.read_json(&["config", "ls", "--json"], project)
    }

    /// `mise tasks ls --json` for a project.
    pub fn tasks(&self, project: &Path) -> Result<Vec<Task>, MiseAdapterError> {
        self.read_json(&["tasks", "ls", "--json"], Some(project))
    }

    /// `mise env --json` for a project.
    pub fn env(
        &self,
        project: &Path,
    ) -> Result<std::collections::BTreeMap<String, String>, MiseAdapterError> {
        self.read_json(&["env", "--json"], Some(project))
    }

    /// `mise doctor --json`.
    pub fn doctor(&self) -> Result<Doctor, MiseAdapterError> {
        self.read_json(&["doctor", "--json"], None)
    }

    /// Runs a change, streaming each output line to `on_line` until it exits
    /// or `cancel` is set (which kills it).
    pub fn run(
        &self,
        command: &MiseCommand,
        cancel: &AtomicBool,
        mut on_line: impl FnMut(OutputLine),
    ) -> Result<RunOutcome, MiseAdapterError> {
        let mut process = self.base(command.cwd.as_deref());
        process
            .args(&command.arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Its own process group, so cancelling also stops the downloads and
        // extractors mise starts.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut process, 0);
        let mut child = process
            .spawn()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })?;

        let (sender, receiver) = mpsc::channel();
        let readers = [
            child
                .stdout
                .take()
                .map(|pipe| forward(pipe, Stream::Stdout, sender.clone())),
            child
                .stderr
                .take()
                .map(|pipe| forward(pipe, Stream::Stderr, sender.clone())),
        ];
        drop(sender);

        let mut cancelled = false;
        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => on_line(line),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                // Both pipes closed: the process has exited.
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    for reader in readers.into_iter().flatten() {
                        let _ = reader.join();
                    }
                    break;
                }
            }
            if cancel.load(Ordering::Relaxed) {
                cancelled = true;
                terminate(&mut child);
                // Don't wait on the pipes: a grandchild can hold them open.
                // The detached reader threads end when it exits.
                break;
            }
        }
        let status = child.wait().map_err(|source| MiseAdapterError::Execute {
            executable: self.executable.clone(),
            source,
        })?;
        Ok(RunOutcome {
            success: status.success() && !cancelled,
            code: status.code(),
            cancelled,
        })
    }
}

/// Stops a running command and everything it started.
fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The child leads its own process group (see `run`).
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

fn forward(
    pipe: impl Read + Send + 'static,
    stream: Stream,
    sender: mpsc::Sender<OutputLine>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for text in BufReader::new(pipe).lines().map_while(Result::ok) {
            if sender.send(OutputLine { stream, text }).is_err() {
                break;
            }
        }
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
}

/// One line of a running command's output.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct OutputLine {
    pub stream: Stream,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RunOutcome {
    pub success: bool,
    pub code: Option<i32>,
    pub cancelled: bool,
}

/// A mise command that changes state, built from validated parts so its
/// rendered form is exactly what runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiseCommand {
    arguments: Vec<String>,
    cwd: Option<PathBuf>,
}

impl MiseCommand {
    fn new(arguments: Vec<String>) -> Self {
        Self {
            arguments,
            cwd: None,
        }
    }

    /// Runs the command inside a project directory.
    pub fn in_project(mut self, project: impl Into<PathBuf>) -> Self {
        self.cwd = Some(project.into());
        self
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    /// The command line as a person would type it.
    pub fn display(&self) -> String {
        let mut line = String::from("mise");
        for argument in &self.arguments {
            line.push(' ');
            if argument.contains(char::is_whitespace) || argument.is_empty() {
                line.push_str(&format!("'{argument}'"));
            } else {
                line.push_str(argument);
            }
        }
        if let Some(cwd) = &self.cwd {
            format!("cd {} && {line}", shell_quote(cwd))
        } else {
            line
        }
    }

    /// `mise use -g <tool>@<version>`: install and set as the global default.
    pub fn use_global(tool: &str, version: &str) -> Result<Self, MiseAdapterError> {
        Ok(Self::new(vec![
            "use".into(),
            "-g".into(),
            tool_spec(tool, version)?,
        ]))
    }

    /// `mise unuse -g <tool>`: remove from the global config (and prune it).
    pub fn unuse_global(tool: &str) -> Result<Self, MiseAdapterError> {
        validate_argument(tool)?;
        Ok(Self::new(vec!["unuse".into(), "-g".into(), tool.into()]))
    }

    /// `mise uninstall <tool>@<version>`: remove one installed version.
    pub fn uninstall(tool: &str, version: &str) -> Result<Self, MiseAdapterError> {
        Ok(Self::new(vec![
            "uninstall".into(),
            tool_spec(tool, version)?,
        ]))
    }

    /// `mise upgrade --bump [tool]`: upgrade one tool, or all when `None`.
    /// `--bump` moves past pinned versions and updates the config to match.
    pub fn upgrade(tool: Option<&str>) -> Result<Self, MiseAdapterError> {
        let mut arguments = vec!["upgrade".into(), "--bump".into()];
        if let Some(tool) = tool {
            validate_argument(tool)?;
            arguments.push(tool.into());
        }
        Ok(Self::new(arguments))
    }

    /// `mise install`: install everything the current config requests.
    pub fn install_all() -> Self {
        Self::new(vec!["install".into()])
    }

    /// `mise self-update --yes`: update mise itself (the person already
    /// confirmed in the app).
    pub fn self_update() -> Self {
        Self::new(vec!["self-update".into(), "--yes".into()])
    }
}

fn tool_spec(tool: &str, version: &str) -> Result<String, MiseAdapterError> {
    validate_argument(tool)?;
    validate_argument(version)?;
    Ok(format!("{tool}@{version}"))
}

/// Tool names and versions come from people and from the registry. They are
/// passed as discrete arguments (never through a shell), so the only risk is
/// one being read as a flag.
fn validate_argument(value: &str) -> Result<(), MiseAdapterError> {
    let valid = !value.is_empty()
        && !value.starts_with('-')
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control());
    if valid {
        Ok(())
    } else {
        Err(MiseAdapterError::InvalidArgument {
            value: value.into(),
        })
    }
}

fn shell_quote(path: &Path) -> String {
    let text = path.display().to_string();
    if text
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "/._-~".contains(character))
    {
        text
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_render_exactly_what_runs() {
        let command = MiseCommand::use_global("node", "22.11.0").unwrap();
        assert_eq!(command.arguments(), ["use", "-g", "node@22.11.0"]);
        assert_eq!(command.display(), "mise use -g node@22.11.0");
        assert_eq!(
            MiseCommand::upgrade(None).unwrap().display(),
            "mise upgrade --bump"
        );
        assert_eq!(
            MiseCommand::upgrade(Some("jq")).unwrap().display(),
            "mise upgrade --bump jq"
        );
        assert_eq!(
            MiseCommand::uninstall("jq", "1.7.1").unwrap().display(),
            "mise uninstall jq@1.7.1"
        );
        assert_eq!(
            MiseCommand::install_all()
                .in_project("/Users/demo/My Project")
                .display(),
            "cd '/Users/demo/My Project' && mise install"
        );
    }

    #[test]
    fn arguments_that_look_like_flags_are_rejected() {
        for value in ["--force", "-g", "", "node 22", "a\nb"] {
            assert!(matches!(
                MiseCommand::unuse_global(value),
                Err(MiseAdapterError::InvalidArgument { .. })
            ));
        }
        assert!(MiseCommand::use_global("npm:@openai/codex", "0.51.0").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn run_streams_output_and_reports_exit() {
        // `sh` stands in for mise: the client only cares about process I/O.
        let client = MiseClient::new("sh", std::env::temp_dir());
        let command = MiseCommand::new(vec!["-c".into(), "echo one; echo two >&2; exit 3".into()]);
        let mut lines = Vec::new();
        let outcome = client
            .run(&command, &AtomicBool::new(false), |line| lines.push(line))
            .unwrap();
        assert_eq!(outcome.code, Some(3));
        assert!(!outcome.success);
        assert!(lines.contains(&OutputLine {
            stream: Stream::Stdout,
            text: "one".into()
        }));
        assert!(lines.contains(&OutputLine {
            stream: Stream::Stderr,
            text: "two".into()
        }));
    }

    #[cfg(unix)]
    #[test]
    fn run_cancels_by_killing_the_process() {
        let client = MiseClient::new("sh", std::env::temp_dir());
        let command = MiseCommand::new(vec!["-c".into(), "echo started; sleep 30".into()]);
        let cancel = AtomicBool::new(false);
        let started = std::time::Instant::now();
        let outcome = client
            .run(&command, &cancel, |_| cancel.store(true, Ordering::Relaxed))
            .unwrap();
        assert!(outcome.cancelled);
        assert!(!outcome.success);
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
